use crate::compose::{decompose_final, decompose_vowel};
use crate::{HangulComposer, InputResult};

pub fn backspace(composer: &mut HangulComposer) -> InputResult {
    if let Some(final_jamo) = composer.final_jamo {
        composer.final_jamo = decompose_final(final_jamo).map(|(left, _)| left);
        return composer.changed();
    }

    if let Some(vowel) = composer.vowel {
        composer.vowel = decompose_vowel(vowel).map(|(left, _)| left);
        return composer.changed();
    }

    if composer.initial.take().is_some() {
        return InputResult::Clear;
    }

    InputResult::Clear
}
