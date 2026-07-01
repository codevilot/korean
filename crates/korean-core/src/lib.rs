pub mod backspace;
pub mod compose;
pub mod keymap_2beolsik;
pub mod syllable;

use compose::{combine_final, combine_vowel, decompose_final};
use keymap_2beolsik::{map_key, Jamo};
use syllable::{compose_syllable, jongseong_index};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputResult {
    PreeditChanged { preedit: String },
    Commit { text: String },
    CommitAndPreedit { commit: String, preedit: String },
    Clear,
}

#[derive(Clone, Debug, Default)]
pub struct HangulComposer {
    pub(crate) initial: Option<char>,
    pub(crate) vowel: Option<char>,
    pub(crate) final_jamo: Option<char>,
}

impl HangulComposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input_key(&mut self, key: char) -> InputResult {
        match map_key(key) {
            Some(Jamo::Consonant(c)) => self.input_consonant(c),
            Some(Jamo::Vowel(v)) => self.input_vowel(v),
            None => {
                let mut text = self.commit().unwrap_or_default();
                text.push(key);
                InputResult::Commit { text }
            }
        }
    }

    pub fn backspace(&mut self) -> InputResult {
        backspace::backspace(self)
    }

    pub fn commit(&mut self) -> Option<String> {
        let text = self.preedit();
        self.reset();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn preedit(&self) -> String {
        match (self.initial, self.vowel, self.final_jamo) {
            (Some(i), Some(v), f) => compose_syllable(i, v, f).unwrap_or(i).to_string(),
            (Some(i), None, None) => i.to_string(),
            (None, Some(v), None) => v.to_string(),
            _ => String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.initial = None;
        self.vowel = None;
        self.final_jamo = None;
    }

    pub(crate) fn changed(&self) -> InputResult {
        let preedit = self.preedit();
        if preedit.is_empty() {
            InputResult::Clear
        } else {
            InputResult::PreeditChanged { preedit }
        }
    }

    fn input_consonant(&mut self, c: char) -> InputResult {
        match (self.initial, self.vowel, self.final_jamo) {
            (None, None, None) => {
                self.initial = Some(c);
                self.changed()
            }
            (Some(_), None, None) | (None, Some(_), None) => {
                let commit = self.commit().unwrap_or_default();
                self.initial = Some(c);
                InputResult::CommitAndPreedit {
                    commit,
                    preedit: self.preedit(),
                }
            }
            (Some(_), Some(_), None) => {
                if jongseong_index(c).is_some() {
                    self.final_jamo = Some(c);
                    self.changed()
                } else {
                    let commit = self.commit().unwrap_or_default();
                    self.initial = Some(c);
                    InputResult::CommitAndPreedit {
                        commit,
                        preedit: self.preedit(),
                    }
                }
            }
            (Some(_), Some(_), Some(f)) => {
                if let Some(combined) = combine_final(f, c) {
                    self.final_jamo = Some(combined);
                    self.changed()
                } else {
                    let commit = self.commit().unwrap_or_default();
                    self.initial = Some(c);
                    InputResult::CommitAndPreedit {
                        commit,
                        preedit: self.preedit(),
                    }
                }
            }
            _ => self.changed(),
        }
    }

    fn input_vowel(&mut self, v: char) -> InputResult {
        match (self.initial, self.vowel, self.final_jamo) {
            (None, None, None) => {
                self.vowel = Some(v);
                self.changed()
            }
            (Some(_), None, None) => {
                self.vowel = Some(v);
                self.changed()
            }
            (None, Some(current), None) | (Some(_), Some(current), None) => {
                if let Some(combined) = combine_vowel(current, v) {
                    self.vowel = Some(combined);
                    self.changed()
                } else {
                    let commit = self.commit().unwrap_or_default();
                    self.vowel = Some(v);
                    InputResult::CommitAndPreedit {
                        commit,
                        preedit: self.preedit(),
                    }
                }
            }
            (Some(_), Some(_), Some(f)) => {
                let (remaining_final, moved_initial) = match decompose_final(f) {
                    Some((left, right)) => (Some(left), right),
                    None => (None, f),
                };
                self.final_jamo = remaining_final;
                let commit = self.commit().unwrap_or_default();
                self.initial = Some(moved_initial);
                self.vowel = Some(v);
                InputResult::CommitAndPreedit {
                    commit,
                    preedit: self.preedit(),
                }
            }
            _ => self.changed(),
        }
    }
}
