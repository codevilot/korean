use super::keys::{
    IBUS_CAP_SURROUNDING_TEXT, IBUS_CONTROL_MASK, IBUS_LOCK_MASK, IBUS_RELEASE_MASK,
    IBUS_SHIFT_MASK, KEY_ALT_L, KEY_BACKSPACE, KEY_CAPS_LOCK, KEY_CONTROL_L, KEY_ESCAPE,
    KEY_RETURN, KEY_RIGHT, KEY_SHIFT_L, KEY_SHIFT_R, KEY_SPACE, KEY_TAB,
};
use super::{
    en_key_for_modifiers, is_modifier_key, ko_key_for_modifiers, process_key_event_with_ops,
    DeleteMode, EngineState, IbusOps, RenderMode,
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
fn english_mode_forwards_tab_and_navigation_keys() {
    let mut state = state(RenderMode::DelayedPreview);
    let mut ops = FakeOps::default();

    assert!(pass_key(&mut state, &mut ops, KEY_CAPS_LOCK));
    assert!(pass_key(&mut state, &mut ops, KEY_TAB));
    assert!(pass_key(&mut state, &mut ops, KEY_RIGHT));

    assert!(ops.events.contains(&Event::Forward(KEY_TAB, 0)));
    assert!(ops.events.contains(&Event::Forward(KEY_RIGHT, 0)));
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
