pub const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

pub const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

pub const JONGSEONG: [Option<char>; 28] = [
    None,
    Some('ㄱ'),
    Some('ㄲ'),
    Some('ㄳ'),
    Some('ㄴ'),
    Some('ㄵ'),
    Some('ㄶ'),
    Some('ㄷ'),
    Some('ㄹ'),
    Some('ㄺ'),
    Some('ㄻ'),
    Some('ㄼ'),
    Some('ㄽ'),
    Some('ㄾ'),
    Some('ㄿ'),
    Some('ㅀ'),
    Some('ㅁ'),
    Some('ㅂ'),
    Some('ㅄ'),
    Some('ㅅ'),
    Some('ㅆ'),
    Some('ㅇ'),
    Some('ㅈ'),
    Some('ㅊ'),
    Some('ㅋ'),
    Some('ㅌ'),
    Some('ㅍ'),
    Some('ㅎ'),
];

pub fn choseong_index(jamo: char) -> Option<usize> {
    CHOSEONG.iter().position(|&c| c == jamo)
}

pub fn jungseong_index(jamo: char) -> Option<usize> {
    JUNGSEONG.iter().position(|&c| c == jamo)
}

pub fn jongseong_index(jamo: char) -> Option<usize> {
    JONGSEONG.iter().position(|&c| c == Some(jamo))
}

pub fn compose_syllable(initial: char, vowel: char, final_jamo: Option<char>) -> Option<char> {
    let l = choseong_index(initial)?;
    let v = jungseong_index(vowel)?;
    let t = match final_jamo {
        Some(c) => jongseong_index(c)?,
        None => 0,
    };
    char::from_u32(0xAC00 + ((l as u32 * 21 + v as u32) * 28 + t as u32))
}
