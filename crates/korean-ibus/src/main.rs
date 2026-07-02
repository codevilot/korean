use std::ffi::{c_char, c_int, c_void, CString};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;

use korean_core::{syllable::compose_syllable, HangulComposer, InputResult};
use korean_state::{InputMode, ModeState};

const COMPONENT_XML: &str = include_str!("../../../data/ibus/korean.xml");

const IBUS_SHIFT_MASK: u32 = 1 << 0;
#[cfg(test)]
const IBUS_LOCK_MASK: u32 = 1 << 1;
const IBUS_CONTROL_MASK: u32 = 1 << 2;
const IBUS_MOD1_MASK: u32 = 1 << 3;
const IBUS_RELEASE_MASK: u32 = 1 << 30;
const IBUS_CAP_SURROUNDING_TEXT: u32 = 1 << 5;

const KEY_BACKSPACE: u32 = 0xff08;
const KEYCODE_BACKSPACE: u32 = 14;
const KEY_RETURN: u32 = 0xff0d;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_CAPS_LOCK: u32 = 0xffe5;
const KEY_CONTROL_L: u32 = 0xffe3;
const KEY_CONTROL_R: u32 = 0xffe4;
const KEY_SHIFT_L: u32 = 0xffe1;
const KEY_SHIFT_R: u32 = 0xffe2;
const KEY_ALT_L: u32 = 0xffe9;
const KEY_ALT_R: u32 = 0xffea;
const KEY_SPACE: u32 = 0x020;
const KEY_TAB: u32 = 0xff09;
const KEY_LEFT: u32 = 0xff51;
const KEY_UP: u32 = 0xff52;
const KEY_RIGHT: u32 = 0xff53;
const KEY_DOWN: u32 = 0xff54;

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
    fn bk_ibus_forward_key_event(engine: *mut c_void, keyval: u32, keycode: u32, modifiers: u32);
    fn bk_ibus_delete_surrounding_text(engine: *mut c_void, offset_from_cursor: i32, nchars: u32);
    fn bk_ibus_surrounding_ends_with(engine: *mut c_void, suffix: *const c_char) -> c_int;
}

struct EngineState {
    composer: HangulComposer,
    modes: ModeState,
    render_mode: RenderMode,
    rendered_text: String,
    delete_mode: DeleteMode,
    capabilities: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderMode {
    VisibleTail,
    Delayed,
    DelayedPreview,
    Preedit,
    Safe,
}

impl RenderMode {
    fn from_env() -> Self {
        match std::env::var("KOREAN_RENDER_MODE").as_deref() {
            Ok("preedit") => Self::Preedit,
            Ok("safe") => Self::Safe,
            Ok("delayed") => Self::Delayed,
            Ok("delayed_preview") => Self::DelayedPreview,
            Ok("visible_tail") => Self::VisibleTail,
            Err(_) => Self::DelayedPreview,
            Ok(other) => {
                debug_log(format_args!(
                    "unknown render_mode={other}, falling back to delayed_preview"
                ));
                Self::DelayedPreview
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeleteMode {
    Backspace,
    BackspacePair,
    Surrounding,
}

impl DeleteMode {
    fn from_env() -> Self {
        match std::env::var("KOREAN_DELETE_MODE").as_deref() {
            Ok("surrounding") | Err(_) => Self::Surrounding,
            Ok("backspace_press") => Self::Backspace,
            Ok("backspace_pair") => Self::BackspacePair,
            Ok(other) => {
                debug_log(format_args!(
                    "unknown delete_mode={other}, falling back to surrounding"
                ));
                Self::Surrounding
            }
        }
    }
}

trait IbusOps {
    fn commit_text(&mut self, text: &str);
    fn update_preedit(&mut self, preedit: &str);
    fn clear_preedit(&mut self);
    fn forward_key_event(&mut self, keyval: u32, keycode: u32, modifiers: u32);
    fn delete_surrounding_text(&mut self, offset_from_cursor: i32, nchars: u32);
    fn surrounding_ends_with(&mut self, suffix: &str) -> bool;
}

struct RealIbusOps {
    engine: *mut c_void,
}

impl IbusOps for RealIbusOps {
    fn commit_text(&mut self, text: &str) {
        commit_text(self.engine, text);
    }

    fn update_preedit(&mut self, preedit: &str) {
        update_preedit(self.engine, preedit);
    }

    fn clear_preedit(&mut self) {
        clear_preedit(self.engine);
    }

    fn forward_key_event(&mut self, keyval: u32, keycode: u32, modifiers: u32) {
        unsafe {
            bk_ibus_forward_key_event(self.engine, keyval, keycode, modifiers);
        }
    }

    fn delete_surrounding_text(&mut self, offset_from_cursor: i32, nchars: u32) {
        unsafe {
            bk_ibus_delete_surrounding_text(self.engine, offset_from_cursor, nchars);
        }
    }

    fn surrounding_ends_with(&mut self, suffix: &str) -> bool {
        let c_suffix = CString::new(suffix).expect("composition text does not contain nul bytes");
        unsafe { bk_ibus_surrounding_ends_with(self.engine, c_suffix.as_ptr()) != 0 }
    }
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
        render_mode: RenderMode::from_env(),
        rendered_text: String::new(),
        delete_mode: DeleteMode::from_env(),
        capabilities: 0,
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
    let mut ops = RealIbusOps { engine };
    reset_state(state, &mut ops);
}

#[no_mangle]
extern "C" fn bk_engine_process_key_event(
    state: *mut c_void,
    engine: *mut c_void,
    keyval: u32,
    keycode: u32,
    modifiers: u32,
) -> c_int {
    let Some(state) = engine_state(state) else {
        return 0;
    };
    let mut ops = RealIbusOps { engine };
    process_key_event_with_ops(state, &mut ops, keyval, keycode, modifiers) as c_int
}

#[no_mangle]
extern "C" fn bk_engine_set_capabilities(state: *mut c_void, capabilities: u32) {
    let Some(state) = engine_state(state) else {
        return;
    };
    state.capabilities = capabilities;
    debug_log(format_args!(
        "set_capabilities=0x{capabilities:x} supports_surrounding={}",
        supports_surrounding_text(state)
    ));
}

fn process_key_event_with_ops(
    state: &mut EngineState,
    ops: &mut impl IbusOps,
    keyval: u32,
    keycode: u32,
    modifiers: u32,
) -> bool {
    debug_log(format_args!(
        "key render_mode={:?} effective_render_mode={:?} delete_mode={:?} caps=0x{:x} keyval=0x{keyval:x} keycode={keycode} modifiers=0x{modifiers:x} mode={:?} rendered_before={:?} composer_before={:?}",
        state.render_mode,
        effective_render_mode(state),
        state.delete_mode,
        state.capabilities,
        state.modes.mode(),
        state.rendered_text,
        state.composer.preedit()
    ));

    if modifiers & IBUS_RELEASE_MASK != 0 {
        debug_log(format_args!(
            "release keyval=0x{keyval:x} handled={}",
            keyval == KEY_CAPS_LOCK
        ));
        return keyval == KEY_CAPS_LOCK;
    }

    if keyval == KEY_CAPS_LOCK {
        commit_composition(state, ops, CommitTrigger::CapsLock);
        if modifiers & IBUS_SHIFT_MASK != 0 {
            state.modes.shift_caps();
        } else {
            state.modes.caps_tap();
        }
        debug_log(format_args!("caps mode={:?}", state.modes.mode()));
        ops.clear_preedit();
        return true;
    }

    if is_modifier_key(keyval) {
        return false;
    }

    if keyval == KEY_ESCAPE {
        return process_escape(state, ops);
    }

    if keyval == KEY_BACKSPACE {
        return process_backspace(state, ops, keycode, modifiers);
    }

    match state.modes.mode() {
        InputMode::En | InputMode::EnCaps => process_en(ops, keyval, modifiers),
        InputMode::Ko => process_ko(state, ops, keyval, keycode, modifiers),
    }
}

fn process_en(ops: &mut impl IbusOps, keyval: u32, modifiers: u32) -> bool {
    if has_command_modifier(modifiers) {
        return false;
    }
    if let Some(ch) = ascii_letter(keyval) {
        let out = en_key_for_modifiers(ch, modifiers);
        ops.commit_text(&out.to_string());
        return true;
    }
    false
}

fn process_ko(
    state: &mut EngineState,
    ops: &mut impl IbusOps,
    keyval: u32,
    _keycode: u32,
    modifiers: u32,
) -> bool {
    if has_command_modifier(modifiers) {
        if effective_render_mode(state) != RenderMode::Preedit {
            commit_composition(state, ops, CommitTrigger::Command);
        }
        return false;
    }

    if let Some(trigger) = commit_trigger(keyval) {
        commit_composition(state, ops, trigger);
        return false;
    }

    let Some(ch) = ascii_letter(keyval) else {
        commit_composition(state, ops, CommitTrigger::Other);
        return false;
    };

    let key = ko_key_for_modifiers(ch, modifiers);
    debug_log(format_args!(
        "ko input normalized_key={key} raw_consumed=true"
    ));
    match effective_render_mode(state) {
        RenderMode::Preedit => apply_preedit_result(ops, state.composer.input_key(key)),
        RenderMode::VisibleTail => apply_visible_tail_key(state, ops, key),
        RenderMode::Delayed => apply_delayed_key(state, ops, key),
        RenderMode::DelayedPreview => apply_delayed_preview_key(state, ops, key),
        RenderMode::Safe => {
            let result = state.composer.input_key(key);
            debug_log(format_args!("safe result={result:?} rendered_after=\"\""));
        }
    }
    true
}

fn ko_key_for_modifiers(ch: char, modifiers: u32) -> char {
    if modifiers & IBUS_SHIFT_MASK != 0 && !has_command_modifier(modifiers) {
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

fn apply_preedit_result(ops: &mut impl IbusOps, result: InputResult) {
    debug_log(format_args!("result={result:?}"));
    match result {
        InputResult::PreeditChanged { preedit } => ops.update_preedit(&preedit),
        InputResult::Commit { text } => {
            ops.commit_text(&text);
            ops.clear_preedit();
        }
        InputResult::CommitAndPreedit { commit, preedit } => {
            ops.commit_text(&commit);
            ops.update_preedit(&preedit);
        }
        InputResult::Clear => ops.clear_preedit(),
    }
}

fn apply_visible_tail_key(state: &mut EngineState, ops: &mut impl IbusOps, key: char) {
    if !delete_rendered_text(state, ops) {
        debug_log(format_args!(
            "visible_tail unsafe_delete rendered_text={:?}; switching event to safe composition",
            state.rendered_text
        ));
        let result = state.composer.input_key(key);
        debug_log(format_args!("visible_tail unsafe_delete result={result:?}"));
        return;
    }
    let result = state.composer.input_key(key);
    debug_log(format_args!("visible_tail result={result:?}"));
    match result {
        InputResult::PreeditChanged { preedit } => render_tail(state, ops, &preedit),
        InputResult::Commit { text } => {
            let text = visible_tail_text(&text);
            ops.commit_text(&text);
            state.rendered_text.clear();
            ops.clear_preedit();
            debug_log(format_args!("commit_text={text:?} rendered_after=\"\""));
        }
        InputResult::CommitAndPreedit { commit, preedit } => {
            let commit = visible_tail_text(&commit);
            ops.commit_text(&commit);
            render_tail(state, ops, &preedit);
        }
        InputResult::Clear => {
            state.rendered_text.clear();
            ops.clear_preedit();
        }
    }
}

fn apply_delayed_key(state: &mut EngineState, ops: &mut impl IbusOps, key: char) {
    let result = state.composer.input_key(key);
    debug_log(format_args!("delayed result={result:?}"));
    match result {
        InputResult::PreeditChanged { preedit } => {
            debug_log(format_args!("delayed hold_preedit={preedit:?}"));
        }
        InputResult::Commit { text } => {
            commit_delayed_text(ops, &text);
        }
        InputResult::CommitAndPreedit { commit, preedit } => {
            commit_delayed_text(ops, &commit);
            debug_log(format_args!("delayed hold_preedit={preedit:?}"));
        }
        InputResult::Clear => {}
    }
    state.rendered_text.clear();
}

fn apply_delayed_preview_key(state: &mut EngineState, ops: &mut impl IbusOps, key: char) {
    let result = state.composer.input_key(key);
    debug_log(format_args!("delayed_preview result={result:?}"));
    match result {
        InputResult::PreeditChanged { preedit } => {
            ops.update_preedit(&visible_tail_text(&preedit));
        }
        InputResult::Commit { text } => {
            commit_delayed_text(ops, &text);
            ops.clear_preedit();
        }
        InputResult::CommitAndPreedit { commit, preedit } => {
            commit_delayed_text(ops, &commit);
            ops.update_preedit(&visible_tail_text(&preedit));
        }
        InputResult::Clear => ops.clear_preedit(),
    }
    state.rendered_text.clear();
}

fn process_backspace(
    state: &mut EngineState,
    ops: &mut impl IbusOps,
    _keycode: u32,
    _modifiers: u32,
) -> bool {
    match effective_render_mode(state) {
        RenderMode::Preedit => {
            if !state.composer.preedit().is_empty() {
                commit_composition(state, ops, CommitTrigger::Other);
            }
            false
        }
        RenderMode::VisibleTail => {
            if state.rendered_text.is_empty() && state.composer.preedit().is_empty() {
                return false;
            }
            if !delete_rendered_text(state, ops) {
                state.composer.reset();
                state.rendered_text.clear();
                return true;
            }
            let result = state.composer.backspace();
            debug_log(format_args!("backspace result={result:?}"));
            match result {
                InputResult::PreeditChanged { preedit } => render_tail(state, ops, &preedit),
                InputResult::Clear => state.rendered_text.clear(),
                InputResult::Commit { text } => ops.commit_text(&visible_tail_text(&text)),
                InputResult::CommitAndPreedit { commit, preedit } => {
                    let commit = visible_tail_text(&commit);
                    ops.commit_text(&commit);
                    render_tail(state, ops, &preedit);
                }
            }
            true
        }
        RenderMode::Delayed => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            let result = state.composer.backspace();
            debug_log(format_args!("delayed backspace result={result:?}"));
            true
        }
        RenderMode::DelayedPreview => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            if let Some(text) = state.composer.commit() {
                commit_delayed_text(ops, &text);
            }
            ops.clear_preedit();
            state.rendered_text.clear();
            debug_log(format_args!(
                "delayed_preview backspace committed_preview_and_forwarded"
            ));
            ops.forward_key_event(KEY_BACKSPACE, KEYCODE_BACKSPACE, 0);
            true
        }
        RenderMode::Safe => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            let result = state.composer.backspace();
            debug_log(format_args!("safe backspace result={result:?}"));
            true
        }
    }
}

fn process_escape(state: &mut EngineState, ops: &mut impl IbusOps) -> bool {
    match effective_render_mode(state) {
        RenderMode::Preedit => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            state.composer.reset();
            ops.clear_preedit();
            true
        }
        RenderMode::VisibleTail => {
            if state.rendered_text.is_empty() && state.composer.preedit().is_empty() {
                return false;
            }
            if !delete_rendered_text(state, ops) {
                state.composer.reset();
                state.rendered_text.clear();
                return true;
            }
            state.composer.reset();
            state.rendered_text.clear();
            ops.clear_preedit();
            true
        }
        RenderMode::Delayed => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            state.composer.reset();
            true
        }
        RenderMode::DelayedPreview => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            state.composer.reset();
            ops.clear_preedit();
            true
        }
        RenderMode::Safe => {
            if state.composer.preedit().is_empty() {
                return false;
            }
            state.composer.reset();
            true
        }
    }
}

fn commit_composition(state: &mut EngineState, ops: &mut impl IbusOps, trigger: CommitTrigger) {
    debug_log(format_args!(
        "commit_trigger={trigger:?} rendered_before={:?} composer_before={:?}",
        state.rendered_text,
        state.composer.preedit()
    ));
    match effective_render_mode(state) {
        RenderMode::VisibleTail => {
            // The tail is already inserted in the application, so committing only
            // means accepting it internally. Re-committing here would duplicate it.
            state.composer.reset();
            state.rendered_text.clear();
            ops.clear_preedit();
        }
        RenderMode::Preedit => {
            if let Some(text) = state.composer.commit() {
                ops.commit_text(&text);
                debug_log(format_args!("commit_text={text:?}"));
            }
            ops.clear_preedit();
            state.rendered_text.clear();
        }
        RenderMode::Delayed | RenderMode::Safe => {
            if let Some(text) = state.composer.commit() {
                commit_delayed_text(ops, &text);
            }
            state.rendered_text.clear();
        }
        RenderMode::DelayedPreview => {
            if let Some(text) = state.composer.commit() {
                commit_delayed_text(ops, &text);
            }
            ops.clear_preedit();
            state.rendered_text.clear();
        }
    }
}

fn reset_state(state: &mut EngineState, ops: &mut impl IbusOps) {
    commit_composition(state, ops, CommitTrigger::FocusOut);
}

fn effective_render_mode(state: &EngineState) -> RenderMode {
    if state.render_mode == RenderMode::VisibleTail
        && state.delete_mode == DeleteMode::Surrounding
        && !supports_surrounding_text(state)
    {
        RenderMode::DelayedPreview
    } else {
        state.render_mode
    }
}

fn supports_surrounding_text(state: &EngineState) -> bool {
    state.capabilities & IBUS_CAP_SURROUNDING_TEXT != 0
}

fn render_tail(state: &mut EngineState, ops: &mut impl IbusOps, text: &str) {
    let text = visible_tail_text(text);
    if text.is_empty() {
        state.rendered_text.clear();
        return;
    }
    ops.commit_text(&text);
    debug_log(format_args!("commit_text={text:?}"));
    state.rendered_text = text;
    debug_log(format_args!("rendered_after={:?}", state.rendered_text));
}

fn commit_delayed_text(ops: &mut impl IbusOps, text: &str) {
    let text = visible_tail_text(text);
    if text.is_empty() {
        return;
    }
    ops.commit_text(&text);
    debug_log(format_args!("commit_text={text:?}"));
}

fn visible_tail_text(text: &str) -> String {
    text.chars()
        .map(|ch| compose_syllable('ㅇ', ch, None).unwrap_or(ch))
        .collect()
}

fn delete_rendered_text(state: &mut EngineState, ops: &mut impl IbusOps) -> bool {
    let count = state.rendered_text.chars().count();
    debug_log(format_args!(
        "delete_rendered_text mode={:?} count={count} text={:?}",
        state.delete_mode, state.rendered_text
    ));
    if count == 0 {
        state.rendered_text.clear();
        return true;
    }
    match state.delete_mode {
        DeleteMode::Backspace => {
            for _ in 0..count {
                ops.forward_key_event(KEY_BACKSPACE, KEYCODE_BACKSPACE, 0);
            }
        }
        DeleteMode::BackspacePair => {
            for _ in 0..count {
                ops.forward_key_event(KEY_BACKSPACE, KEYCODE_BACKSPACE, 0);
                ops.forward_key_event(KEY_BACKSPACE, KEYCODE_BACKSPACE, IBUS_RELEASE_MASK);
            }
        }
        DeleteMode::Surrounding => {
            if !ops.surrounding_ends_with(&state.rendered_text) {
                debug_log(format_args!(
                    "delete_rendered_text refused: surrounding cursor does not end with rendered_text={:?}",
                    state.rendered_text
                ));
                return false;
            }
            ops.delete_surrounding_text(-(count as i32), count as u32);
        }
    }
    state.rendered_text.clear();
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitTrigger {
    CapsLock,
    Command,
    Enter,
    Space,
    Tab,
    Arrow,
    Symbol,
    Other,
    FocusOut,
}

fn commit_trigger(keyval: u32) -> Option<CommitTrigger> {
    match keyval {
        KEY_RETURN => Some(CommitTrigger::Enter),
        KEY_SPACE => Some(CommitTrigger::Space),
        KEY_TAB => Some(CommitTrigger::Tab),
        KEY_LEFT | KEY_UP | KEY_RIGHT | KEY_DOWN => Some(CommitTrigger::Arrow),
        _ if ascii_symbol_or_digit(keyval).is_some() => Some(CommitTrigger::Symbol),
        _ => None,
    }
}

fn ascii_symbol_or_digit(keyval: u32) -> Option<char> {
    let ch = char::from_u32(keyval)?;
    if ch.is_ascii_graphic() && !ch.is_ascii_alphabetic() {
        Some(ch)
    } else {
        None
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
    matches!(
        keyval,
        KEY_SHIFT_L | KEY_SHIFT_R | KEY_CONTROL_L | KEY_CONTROL_R | KEY_ALT_L | KEY_ALT_R
    )
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
        en_key_for_modifiers, is_modifier_key, ko_key_for_modifiers, process_key_event_with_ops,
        DeleteMode, EngineState, IbusOps, RenderMode, IBUS_CAP_SURROUNDING_TEXT, IBUS_CONTROL_MASK,
        IBUS_LOCK_MASK, IBUS_RELEASE_MASK, IBUS_SHIFT_MASK, KEY_ALT_L, KEY_BACKSPACE,
        KEY_CAPS_LOCK, KEY_CONTROL_L, KEY_ESCAPE, KEY_RETURN, KEY_RIGHT, KEY_SHIFT_L, KEY_SHIFT_R,
        KEY_SPACE, KEY_TAB,
    };
    use korean_core::HangulComposer;
    use korean_state::ModeState;

    #[derive(Debug, Eq, PartialEq)]
    enum Event {
        Commit(String),
        Preedit(String),
        ClearPreedit,
        Forward(u32, u32),
        DeleteSurrounding(i32, u32),
    }

    struct FakeOps {
        text: String,
        events: Vec<Event>,
        surrounding_matches: bool,
    }

    impl Default for FakeOps {
        fn default() -> Self {
            Self {
                text: String::new(),
                events: Vec::new(),
                surrounding_matches: true,
            }
        }
    }

    impl IbusOps for FakeOps {
        fn commit_text(&mut self, text: &str) {
            self.text.push_str(text);
            self.events.push(Event::Commit(text.to_string()));
        }

        fn update_preedit(&mut self, preedit: &str) {
            self.events.push(Event::Preedit(preedit.to_string()));
        }

        fn clear_preedit(&mut self) {
            self.events.push(Event::ClearPreedit);
        }

        fn forward_key_event(&mut self, keyval: u32, _keycode: u32, modifiers: u32) {
            if keyval == KEY_BACKSPACE && modifiers & IBUS_RELEASE_MASK == 0 {
                self.text.pop();
            }
            self.events.push(Event::Forward(keyval, modifiers));
        }

        fn delete_surrounding_text(&mut self, offset_from_cursor: i32, nchars: u32) {
            if offset_from_cursor == -(nchars as i32) {
                for _ in 0..nchars {
                    self.text.pop();
                }
            }
            self.events
                .push(Event::DeleteSurrounding(offset_from_cursor, nchars));
        }

        fn surrounding_ends_with(&mut self, suffix: &str) -> bool {
            self.surrounding_matches && self.text.ends_with(suffix)
        }
    }

    fn state(render_mode: RenderMode) -> EngineState {
        EngineState {
            composer: HangulComposer::new(),
            modes: ModeState::new(),
            render_mode,
            rendered_text: String::new(),
            delete_mode: DeleteMode::Surrounding,
            capabilities: IBUS_CAP_SURROUNDING_TEXT,
        }
    }

    fn key(state: &mut EngineState, ops: &mut FakeOps, ch: char) -> bool {
        process_key_event_with_ops(state, ops, ch as u32, 0, 0)
    }

    fn shifted_key(state: &mut EngineState, ops: &mut FakeOps, ch: char) -> bool {
        assert!(!process_key_event_with_ops(state, ops, KEY_SHIFT_L, 0, 0));
        process_key_event_with_ops(state, ops, ch as u32, 0, IBUS_SHIFT_MASK)
    }

    fn pass_key(state: &mut EngineState, ops: &mut FakeOps, keyval: u32) -> bool {
        process_key_event_with_ops(state, ops, keyval, 0, 0)
    }

    fn type_visible(input: &str) -> (EngineState, FakeOps, Vec<bool>) {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps {
            surrounding_matches: true,
            ..FakeOps::default()
        };
        let handled = input
            .chars()
            .map(|ch| key(&mut state, &mut ops, ch))
            .collect();
        (state, ops, handled)
    }

    #[test]
    fn korean_mode_uses_shift_modifier_instead_of_keyval_case() {
        assert_eq!(ko_key_for_modifiers('r', 0), 'r');
        assert_eq!(ko_key_for_modifiers('R', 0), 'r');
        assert_eq!(ko_key_for_modifiers('r', IBUS_SHIFT_MASK), 'R');
        assert_eq!(ko_key_for_modifiers('R', IBUS_SHIFT_MASK), 'R');
        assert_eq!(
            ko_key_for_modifiers('R', IBUS_SHIFT_MASK | IBUS_LOCK_MASK),
            'R'
        );
    }

    #[test]
    fn default_render_mode_is_delayed_preview() {
        std::env::remove_var("KOREAN_RENDER_MODE");
        assert_eq!(RenderMode::from_env(), RenderMode::DelayedPreview);
    }

    #[test]
    fn shift_keys_are_modifier_only_events() {
        assert!(is_modifier_key(KEY_SHIFT_L));
        assert!(is_modifier_key(KEY_SHIFT_R));
        assert!(is_modifier_key(KEY_CONTROL_L));
        assert!(is_modifier_key(KEY_ALT_L));
        assert!(!is_modifier_key('a' as u32));
    }

    #[test]
    fn english_mode_is_lowercase_unless_shift_is_held() {
        assert_eq!(en_key_for_modifiers('a', 0), 'a');
        assert_eq!(en_key_for_modifiers('A', 0), 'a');
        assert_eq!(en_key_for_modifiers('a', IBUS_SHIFT_MASK), 'A');
        assert_eq!(en_key_for_modifiers('A', IBUS_SHIFT_MASK), 'A');
    }

    #[test]
    fn visible_tail_composes_basic_words_without_raw_key_leak() {
        for (input, expected) in [
            ("gks", "한"),
            ("gksrmf", "한글"),
            ("gksk", "하나"),
            ("dkssud", "안녕"),
            ("rhk", "과"),
            ("hk", "와"),
            ("nj", "워"),
        ] {
            let (_state, ops, handled) = type_visible(input);
            assert_eq!(ops.text, expected, "{input}");
            assert!(handled.into_iter().all(|handled| handled), "{input}");
            assert!(!ops.text.contains(input), "{input}");
        }
    }

    #[test]
    fn visible_tail_falls_back_to_delayed_preview_without_surrounding_text_capability() {
        let mut state = state(RenderMode::VisibleTail);
        state.capabilities = 0;
        let mut ops = FakeOps::default();

        assert!(key(&mut state, &mut ops, 'r'));
        assert!(key(&mut state, &mut ops, 'k'));

        assert_eq!(ops.text, "");
        assert!(ops.events.contains(&Event::Preedit("가".to_string())));
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));
        assert_eq!(ops.text, "가");
    }

    #[test]
    fn visible_tail_normalizes_caps_lock_uppercase_without_leaking_raw_keys() {
        let (_state, ops, handled) = type_visible("GKS");
        assert_eq!(ops.text, "한");
        assert!(handled.into_iter().all(|handled| handled));
        assert!(!ops.text.contains("GKS"));
    }

    #[test]
    fn visible_tail_uses_shift_mask_when_caps_lock_is_not_active() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'R' as u32,
            0,
            IBUS_SHIFT_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'K' as u32,
            0,
            IBUS_SHIFT_MASK
        ));
        assert_eq!(ops.text, "까");
    }

    #[test]
    fn visible_tail_caps_lock_keeps_actual_shift_for_korean_shift_jamo() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            KEY_SHIFT_L,
            0,
            0
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'R' as u32,
            0,
            IBUS_SHIFT_MASK | IBUS_LOCK_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'K' as u32,
            0,
            IBUS_SHIFT_MASK | IBUS_LOCK_MASK
        ));
        assert_eq!(ops.text, "까");
    }

    #[test]
    fn visible_tail_caps_lock_modifier_with_actual_shift_uses_korean_shift() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            KEY_SHIFT_L,
            0,
            IBUS_LOCK_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'R' as u32,
            0,
            IBUS_SHIFT_MASK | IBUS_LOCK_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'K' as u32,
            0,
            IBUS_SHIFT_MASK | IBUS_LOCK_MASK
        ));
        assert_eq!(ops.text, "까");
    }

    #[test]
    fn shift_pressed_in_english_mode_does_not_leak_without_shift_modifier() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();

        assert!(pass_key(&mut state, &mut ops, KEY_CAPS_LOCK));
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            KEY_SHIFT_L,
            0,
            0
        ));
        assert!(pass_key(&mut state, &mut ops, KEY_CAPS_LOCK));
        assert!(process_key_event_with_ops(
            &mut state, &mut ops, 'R' as u32, 0, 0
        ));
        assert!(process_key_event_with_ops(
            &mut state, &mut ops, 'K' as u32, 0, 0
        ));
        assert_eq!(ops.text, "가");
    }

    #[test]
    fn command_shift_does_not_arm_korean_shift() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();

        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            KEY_SHIFT_L,
            0,
            IBUS_CONTROL_MASK
        ));
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            'V' as u32,
            0,
            IBUS_CONTROL_MASK | IBUS_SHIFT_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state, &mut ops, 'R' as u32, 0, 0
        ));
        assert!(process_key_event_with_ops(
            &mut state, &mut ops, 'K' as u32, 0, 0
        ));
        assert_eq!(ops.text, "가");
    }

    #[test]
    fn visible_tail_shift_does_not_stick_without_shift_modifier() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            KEY_SHIFT_L,
            0,
            0
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'R' as u32,
            0,
            IBUS_SHIFT_MASK
        ));
        assert!(process_key_event_with_ops(
            &mut state,
            &mut ops,
            'K' as u32,
            0,
            IBUS_SHIFT_MASK
        ));
        assert_eq!(ops.text, "까");
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));

        assert!(process_key_event_with_ops(
            &mut state, &mut ops, 'R' as u32, 0, 0
        ));
        assert_eq!(state.rendered_text, "ㄱ");
    }

    #[test]
    fn visible_tail_uses_shift_modifier_for_shifted_jamo() {
        let mut state = state(RenderMode::VisibleTail);
        let mut ops = FakeOps::default();
        assert!(shifted_key(&mut state, &mut ops, 'r'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert_eq!(ops.text, "까");
    }

    #[test]
    fn visible_tail_enter_and_space_do_not_duplicate_rendered_text() {
        let (mut state, mut ops, _) = type_visible("gks");
        assert_eq!(ops.text, "한");
        assert!(!pass_key(&mut state, &mut ops, KEY_RETURN));
        assert_eq!(ops.text, "한");
        assert_eq!(
            ops.events
                .iter()
                .filter(|event| matches!(event, Event::Commit(text) if text == "한"))
                .count(),
            1
        );

        let (mut state, mut ops, _) = type_visible("gks");
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));
        if !ops
            .events
            .last()
            .is_some_and(|event| matches!(event, Event::Commit(_)))
        {
            ops.text.push(' ');
        }
        assert_eq!(ops.text, "한 ");
    }

    #[test]
    fn visible_tail_backspace_edits_composition_before_passing_empty_backspace() {
        let (mut state, mut ops, _) = type_visible("gks");
        assert_eq!(ops.text, "한");

        assert!(pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "하");
        assert!(pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "ㅎ");
        assert!(pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "");
        assert!(!pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "");
    }

    #[test]
    fn visible_tail_can_delete_tail_with_backspace_press_only_mode() {
        let mut state = state(RenderMode::VisibleTail);
        state.delete_mode = DeleteMode::Backspace;
        let mut ops = FakeOps {
            surrounding_matches: true,
            ..FakeOps::default()
        };
        assert!(key(&mut state, &mut ops, 'r'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert_eq!(ops.text, "가");
        assert!(ops.events.contains(&Event::Forward(KEY_BACKSPACE, 0)));
        assert!(!ops
            .events
            .contains(&Event::Forward(KEY_BACKSPACE, IBUS_RELEASE_MASK)));
    }

    #[test]
    fn visible_tail_can_delete_tail_with_backspace_press_and_release_mode() {
        let mut state = state(RenderMode::VisibleTail);
        state.delete_mode = DeleteMode::BackspacePair;
        let mut ops = FakeOps {
            surrounding_matches: true,
            ..FakeOps::default()
        };
        assert!(key(&mut state, &mut ops, 'r'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert_eq!(ops.text, "가");
        assert!(ops.events.windows(2).any(|events| matches!(
            events,
            [
                Event::Forward(KEY_BACKSPACE, 0),
                Event::Forward(KEY_BACKSPACE, IBUS_RELEASE_MASK)
            ]
        )));
    }

    #[test]
    fn visible_tail_backspace_decomposes_final_consonant_and_rerenders_tail() {
        let (mut state, mut ops, _) = type_visible("gksks");
        assert_eq!(ops.text, "하난");

        assert!(pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "하나");
        assert_eq!(state.rendered_text, "나");
    }

    #[test]
    fn visible_tail_can_delete_tail_with_surrounding_text_mode() {
        let mut state = state(RenderMode::VisibleTail);
        state.delete_mode = DeleteMode::Surrounding;
        let mut ops = FakeOps {
            surrounding_matches: true,
            ..FakeOps::default()
        };

        assert!(key(&mut state, &mut ops, 'r'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert_eq!(ops.text, "가");
        assert!(ops.events.contains(&Event::DeleteSurrounding(-1, 1)));
    }

    #[test]
    fn visible_tail_refuses_surrounding_delete_when_cursor_tail_does_not_match() {
        let mut state = state(RenderMode::VisibleTail);
        state.delete_mode = DeleteMode::Surrounding;
        let mut ops = FakeOps {
            surrounding_matches: false,
            ..FakeOps::default()
        };

        assert!(key(&mut state, &mut ops, 'r'));
        assert_eq!(ops.text, "ㄱ");
        assert!(key(&mut state, &mut ops, 'k'));
        assert_eq!(ops.text, "ㄱ");
        assert!(!ops
            .events
            .iter()
            .any(|event| matches!(event, Event::DeleteSurrounding(_, _))));
    }

    #[test]
    fn visible_tail_escape_deletes_tail_and_clears_composer() {
        let (mut state, mut ops, _) = type_visible("gks");
        assert_eq!(ops.text, "한");
        assert!(pass_key(&mut state, &mut ops, KEY_ESCAPE));
        assert_eq!(ops.text, "");
        assert_eq!(state.composer.preedit(), "");
        assert_eq!(state.rendered_text, "");
    }

    #[test]
    fn visible_tail_commit_triggers_accept_tail_and_pass_event() {
        for keyval in [
            KEY_RIGHT, KEY_TAB, '.' as u32, ',' as u32, '!' as u32, '?' as u32,
        ] {
            let (mut state, mut ops, _) = type_visible("gks");
            assert!(!pass_key(&mut state, &mut ops, keyval), "keyval={keyval:x}");
            assert_eq!(ops.text, "한", "keyval={keyval:x}");
            assert_eq!(state.composer.preedit(), "");
            assert_eq!(state.rendered_text, "");
        }
    }

    #[test]
    fn visible_tail_command_modifier_accepts_tail_and_passes_shortcut() {
        let (mut state, mut ops, _) = type_visible("gks");
        assert!(!process_key_event_with_ops(
            &mut state,
            &mut ops,
            'c' as u32,
            0,
            IBUS_CONTROL_MASK
        ));
        assert_eq!(ops.text, "한");
        assert_eq!(state.composer.preedit(), "");
    }

    #[test]
    fn preedit_mode_keeps_update_preedit_flow() {
        let mut state = state(RenderMode::Preedit);
        let mut ops = FakeOps::default();
        assert!(key(&mut state, &mut ops, 'g'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert!(key(&mut state, &mut ops, 's'));
        assert_eq!(ops.text, "");
        assert!(ops.events.contains(&Event::Preedit("한".to_string())));
        assert!(!pass_key(&mut state, &mut ops, KEY_RETURN));
        assert_eq!(ops.text, "한");
    }

    #[test]
    fn delayed_mode_consumes_raw_keys_and_commits_only_stable_text() {
        let mut state = state(RenderMode::Delayed);
        let mut ops = FakeOps::default();

        for ch in "gksk".chars() {
            assert!(key(&mut state, &mut ops, ch));
        }
        assert_eq!(ops.text, "하");
        assert!(!ops
            .events
            .iter()
            .any(|event| matches!(event, Event::Preedit(_))));
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));
        assert_eq!(ops.text, "하나");
    }

    #[test]
    fn delayed_mode_enter_and_space_do_not_duplicate_text() {
        let mut delayed_state = state(RenderMode::Delayed);
        let mut ops = FakeOps::default();

        for ch in "gks".chars() {
            assert!(key(&mut delayed_state, &mut ops, ch));
        }
        assert_eq!(ops.text, "");
        assert!(!pass_key(&mut delayed_state, &mut ops, KEY_RETURN));
        assert_eq!(ops.text, "한");
        assert_eq!(
            ops.events
                .iter()
                .filter(|event| matches!(event, Event::Commit(text) if text == "한"))
                .count(),
            1
        );

        let mut delayed_state = state(RenderMode::Delayed);
        let mut ops = FakeOps::default();
        for ch in "gks".chars() {
            assert!(key(&mut delayed_state, &mut ops, ch));
        }
        assert!(!pass_key(&mut delayed_state, &mut ops, KEY_SPACE));
        assert_eq!(ops.text, "한");
    }

    #[test]
    fn delayed_mode_commits_completed_syllables_during_long_input() {
        let mut state = state(RenderMode::Delayed);
        let mut ops = FakeOps::default();

        for ch in "gksrmf".chars() {
            assert!(key(&mut state, &mut ops, ch));
        }
        assert_eq!(ops.text, "한");
        assert!(!pass_key(&mut state, &mut ops, KEY_RETURN));
        assert_eq!(ops.text, "한글");
    }

    #[test]
    fn delayed_preview_shows_tail_without_committing_until_stable() {
        let mut state = state(RenderMode::DelayedPreview);
        let mut ops = FakeOps::default();

        for ch in "gksk".chars() {
            assert!(key(&mut state, &mut ops, ch));
        }
        assert_eq!(ops.text, "하");
        assert!(ops.events.contains(&Event::Preedit("나".to_string())));
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));
        assert_eq!(ops.text, "하나");
        assert!(ops.events.contains(&Event::ClearPreedit));
    }

    #[test]
    fn delayed_preview_enter_and_space_do_not_duplicate_preview_text() {
        let mut state = state(RenderMode::DelayedPreview);
        let mut ops = FakeOps::default();

        for ch in "gks".chars() {
            assert!(key(&mut state, &mut ops, ch));
        }
        assert_eq!(ops.text, "");
        assert!(ops.events.contains(&Event::Preedit("한".to_string())));
        assert!(!pass_key(&mut state, &mut ops, KEY_RETURN));
        assert_eq!(ops.text, "한");
        assert_eq!(
            ops.events
                .iter()
                .filter(|event| matches!(event, Event::Commit(text) if text == "한"))
                .count(),
            1
        );
    }

    #[test]
    fn delayed_preview_backspace_commits_preview_and_forwards_backspace() {
        let mut state = state(RenderMode::DelayedPreview);
        let mut ops = FakeOps::default();

        for ch in "gksrmf".chars() {
            assert!(key(&mut state, &mut ops, ch));
        }
        assert_eq!(ops.text, "한");
        assert!(ops.events.contains(&Event::Preedit("글".to_string())));

        assert!(pass_key(&mut state, &mut ops, KEY_BACKSPACE));
        assert_eq!(ops.text, "한");
        assert_eq!(state.composer.preedit(), "");
        assert!(ops.events.contains(&Event::ClearPreedit));
        assert!(ops.events.contains(&Event::Forward(KEY_BACKSPACE, 0)));

        assert!(!pass_key(&mut state, &mut ops, KEY_BACKSPACE));
    }

    #[test]
    fn safe_mode_consumes_letters_but_renders_only_on_trigger() {
        let mut state = state(RenderMode::Safe);
        let mut ops = FakeOps::default();
        assert!(key(&mut state, &mut ops, 'g'));
        assert!(key(&mut state, &mut ops, 'k'));
        assert!(key(&mut state, &mut ops, 's'));
        assert_eq!(ops.text, "");
        assert!(!pass_key(&mut state, &mut ops, KEY_SPACE));
        assert_eq!(ops.text, "한");
    }
}
