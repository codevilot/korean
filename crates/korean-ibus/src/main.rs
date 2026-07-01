use std::ffi::{c_char, c_int, c_void, CString};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;

use korean_core::{HangulComposer, InputResult};
use korean_state::{InputMode, ModeState};

const COMPONENT_XML: &str = include_str!("../../../data/ibus/korean.xml");

const IBUS_SHIFT_MASK: u32 = 1 << 0;
const IBUS_CONTROL_MASK: u32 = 1 << 2;
const IBUS_MOD1_MASK: u32 = 1 << 3;
const IBUS_RELEASE_MASK: u32 = 1 << 30;

const KEY_BACKSPACE: u32 = 0xff08;
const KEY_RETURN: u32 = 0xff0d;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_CAPS_LOCK: u32 = 0xffe5;
const KEY_SHIFT_L: u32 = 0xffe1;
const KEY_SHIFT_R: u32 = 0xffe2;
const KEY_SPACE: u32 = 0x020;

#[link(name = "ibus_shim", kind = "static")]
extern "C" {
    fn bk_ibus_run() -> c_int;
    fn bk_ibus_commit_text(engine: *mut c_void, text: *const c_char);
    fn bk_ibus_update_preedit(
        engine: *mut c_void,
        text: *const c_char,
        cursor_pos: u32,
        visible: c_int,
    );
    fn bk_ibus_hide_preedit(engine: *mut c_void);
}

struct EngineState {
    composer: HangulComposer,
    modes: ModeState,
}

fn main() -> ExitCode {
    if std::env::args().any(|arg| arg == "--xml") {
        print!("{COMPONENT_XML}");
        return ExitCode::SUCCESS;
    }

    let code = unsafe { bk_ibus_run() };
    if code == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(code as u8)
    }
}

#[no_mangle]
extern "C" fn bk_engine_state_new() -> *mut c_void {
    Box::into_raw(Box::new(EngineState {
        composer: HangulComposer::new(),
        modes: ModeState::new(),
    })) as *mut c_void
}

#[no_mangle]
extern "C" fn bk_engine_state_free(state: *mut c_void) {
    if !state.is_null() {
        unsafe {
            drop(Box::from_raw(state as *mut EngineState));
        }
    }
}

#[no_mangle]
extern "C" fn bk_engine_reset_state(state: *mut c_void, engine: *mut c_void) {
    let Some(state) = engine_state(state) else {
        return;
    };
    commit_preedit(state, engine);
    clear_preedit(engine);
}

#[no_mangle]
extern "C" fn bk_engine_process_key_event(
    state: *mut c_void,
    engine: *mut c_void,
    keyval: u32,
    _keycode: u32,
    modifiers: u32,
) -> c_int {
    let Some(state) = engine_state(state) else {
        return 0;
    };

    debug_log(format_args!(
        "key keyval=0x{keyval:x} keycode={_keycode} modifiers=0x{modifiers:x} mode={:?}",
        state.modes.mode()
    ));

    if modifiers & IBUS_RELEASE_MASK != 0 {
        debug_log(format_args!(
            "release keyval=0x{keyval:x} handled={}",
            keyval == KEY_CAPS_LOCK
        ));
        return (keyval == KEY_CAPS_LOCK) as c_int;
    }

    if keyval == KEY_CAPS_LOCK {
        commit_preedit(state, engine);
        if modifiers & IBUS_SHIFT_MASK != 0 {
            state.modes.shift_caps();
        } else {
            state.modes.caps_tap();
        }
        debug_log(format_args!("caps mode={:?}", state.modes.mode()));
        clear_preedit(engine);
        return 1;
    }

    if is_modifier_key(keyval) {
        return 0;
    }

    if keyval == KEY_ESCAPE {
        if state.composer.preedit().is_empty() {
            return 0;
        }
        state.composer.reset();
        clear_preedit(engine);
        return 1;
    }

    if keyval == KEY_BACKSPACE {
        if state.composer.preedit().is_empty() {
            return 0;
        }
        apply_result(engine, state.composer.backspace());
        return 1;
    }

    match state.modes.mode() {
        InputMode::En | InputMode::EnCaps => process_en(engine, keyval, modifiers),
        InputMode::Ko => process_ko(state, engine, keyval, modifiers),
    }
}

fn process_en(engine: *mut c_void, keyval: u32, modifiers: u32) -> c_int {
    if has_command_modifier(modifiers) {
        return 0;
    }
    if let Some(ch) = ascii_letter(keyval) {
        let out = en_key_for_modifiers(ch, modifiers);
        commit_text(engine, &out.to_string());
        return 1;
    }
    0
}

fn process_ko(state: &mut EngineState, engine: *mut c_void, keyval: u32, modifiers: u32) -> c_int {
    if has_command_modifier(modifiers) {
        return 0;
    }

    if keyval == KEY_RETURN || keyval == KEY_SPACE {
        commit_preedit(state, engine);
        return 0;
    }

    let Some(ch) = ascii_letter(keyval) else {
        commit_preedit(state, engine);
        return 0;
    };

    let key = ko_key_for_modifiers(ch, modifiers);
    debug_log(format_args!("ko input key={key}"));
    apply_result(engine, state.composer.input_key(key));
    1
}

fn ko_key_for_modifiers(ch: char, modifiers: u32) -> char {
    if ch.is_ascii_uppercase() || modifiers & IBUS_SHIFT_MASK != 0 {
        ch.to_ascii_uppercase()
    } else {
        ch.to_ascii_lowercase()
    }
}

fn en_key_for_modifiers(ch: char, modifiers: u32) -> char {
    if modifiers & IBUS_SHIFT_MASK != 0 {
        ch.to_ascii_uppercase()
    } else {
        ch.to_ascii_lowercase()
    }
}

fn apply_result(engine: *mut c_void, result: InputResult) {
    debug_log(format_args!("result={result:?}"));
    match result {
        InputResult::PreeditChanged { preedit } => update_preedit(engine, &preedit),
        InputResult::Commit { text } => {
            commit_text(engine, &text);
            clear_preedit(engine);
        }
        InputResult::CommitAndPreedit { commit, preedit } => {
            commit_text(engine, &commit);
            update_preedit(engine, &preedit);
        }
        InputResult::Clear => clear_preedit(engine),
    }
}

fn commit_preedit(state: &mut EngineState, engine: *mut c_void) {
    if let Some(text) = state.composer.commit() {
        commit_text(engine, &text);
    }
}

fn commit_text(engine: *mut c_void, text: &str) {
    if text.is_empty() {
        return;
    }
    let c_text = CString::new(text).expect("composition text does not contain nul bytes");
    unsafe {
        bk_ibus_commit_text(engine, c_text.as_ptr());
    }
}

fn update_preedit(engine: *mut c_void, preedit: &str) {
    let c_text = CString::new(preedit).expect("composition text does not contain nul bytes");
    unsafe {
        bk_ibus_update_preedit(engine, c_text.as_ptr(), preedit.chars().count() as u32, 1);
    }
}

fn clear_preedit(engine: *mut c_void) {
    unsafe {
        bk_ibus_hide_preedit(engine);
    }
}

fn ascii_letter(keyval: u32) -> Option<char> {
    char::from_u32(keyval).filter(|ch| ch.is_ascii_alphabetic())
}

fn has_command_modifier(modifiers: u32) -> bool {
    modifiers & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK) != 0
}

fn is_modifier_key(keyval: u32) -> bool {
    matches!(keyval, KEY_SHIFT_L | KEY_SHIFT_R)
}

fn debug_log(args: std::fmt::Arguments<'_>) {
    let Ok(path) = std::env::var("KOREAN_DEBUG_LOG") else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{args}");
}

fn engine_state<'a>(state: *mut c_void) -> Option<&'a mut EngineState> {
    if state.is_null() {
        None
    } else {
        Some(unsafe { &mut *(state as *mut EngineState) })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        en_key_for_modifiers, is_modifier_key, ko_key_for_modifiers, IBUS_SHIFT_MASK, KEY_SHIFT_L,
        KEY_SHIFT_R,
    };

    #[test]
    fn korean_mode_uses_shift_modifier_instead_of_keyval_case() {
        assert_eq!(ko_key_for_modifiers('r', 0), 'r');
        assert_eq!(ko_key_for_modifiers('R', 0), 'R');
        assert_eq!(ko_key_for_modifiers('r', IBUS_SHIFT_MASK), 'R');
        assert_eq!(ko_key_for_modifiers('R', IBUS_SHIFT_MASK), 'R');
    }

    #[test]
    fn shift_keys_are_modifier_only_events() {
        assert!(is_modifier_key(KEY_SHIFT_L));
        assert!(is_modifier_key(KEY_SHIFT_R));
        assert!(!is_modifier_key('a' as u32));
    }

    #[test]
    fn english_mode_is_lowercase_unless_shift_is_held() {
        assert_eq!(en_key_for_modifiers('a', 0), 'a');
        assert_eq!(en_key_for_modifiers('A', 0), 'a');
        assert_eq!(en_key_for_modifiers('a', IBUS_SHIFT_MASK), 'A');
        assert_eq!(en_key_for_modifiers('A', IBUS_SHIFT_MASK), 'A');
    }
}
