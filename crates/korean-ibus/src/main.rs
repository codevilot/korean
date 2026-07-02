use std::ffi::{c_char, c_int, c_void, CString};
use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;

use korean_core::{syllable::compose_syllable, HangulComposer, InputResult};
use korean_state::{InputMode, ModeState};

mod config;
mod keys;

use config::{DeleteMode, RenderMode};
use keys::{
    ascii_letter, ascii_symbol_or_digit, has_command_modifier, is_modifier_key,
    IBUS_CAP_SURROUNDING_TEXT, IBUS_RELEASE_MASK, IBUS_SHIFT_MASK, KEYCODE_BACKSPACE,
    KEY_BACKSPACE, KEY_CAPS_LOCK, KEY_DOWN, KEY_ESCAPE, KEY_LEFT, KEY_RETURN, KEY_RIGHT, KEY_SPACE,
    KEY_TAB, KEY_UP,
};

const COMPONENT_XML: &str = include_str!("../../../data/ibus/korean.xml");

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
mod tests;
