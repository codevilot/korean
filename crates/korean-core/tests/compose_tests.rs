use korean_core::{HangulComposer, InputResult};

fn simulate(input: &str) -> String {
    let mut composer = HangulComposer::new();
    let mut out = String::new();
    for ch in input.chars() {
        match composer.input_key(ch) {
            InputResult::Commit { text } => out.push_str(&text),
            InputResult::CommitAndPreedit { commit, .. } => out.push_str(&commit),
            _ => {}
        }
    }
    if let Some(text) = composer.commit() {
        out.push_str(&text);
    }
    out
}

#[test]
fn composes_basic_words() {
    assert_eq!(simulate("gksrmf"), "한글");
    assert_eq!(simulate("dkssud"), "안녕");
    assert_eq!(simulate("rkskek"), "가나다");
    assert_eq!(simulate("rhk"), "과");
    assert_eq!(simulate("rhkf"), "괄");
    assert_eq!(simulate("rkqt"), "값");
}

#[test]
fn shifted_keys_keep_normal_jamo_unless_2beolsik_changes_them() {
    assert_eq!(simulate("A"), "ㅁ");
    assert_eq!(simulate("akA"), "맘");
    assert_eq!(simulate("K"), "ㅏ");
    assert_eq!(simulate("Rk"), "까");
    assert_eq!(simulate("O"), "ㅒ");
}

#[test]
fn decomposes_with_backspace() {
    let mut composer = HangulComposer::new();
    for ch in "rkqt".chars() {
        composer.input_key(ch);
    }
    assert_eq!(composer.preedit(), "값");

    composer.backspace();
    assert_eq!(composer.preedit(), "갑");
    composer.backspace();
    assert_eq!(composer.preedit(), "가");
    composer.backspace();
    assert_eq!(composer.preedit(), "ㄱ");
    composer.backspace();
    assert_eq!(composer.preedit(), "");
}
