#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Jamo {
    Consonant(char),
    Vowel(char),
}

pub fn map_key(key: char) -> Option<Jamo> {
    let jamo = match key {
        'r' => Jamo::Consonant('ㄱ'),
        'R' => Jamo::Consonant('ㄲ'),
        's' | 'S' => Jamo::Consonant('ㄴ'),
        'e' => Jamo::Consonant('ㄷ'),
        'E' => Jamo::Consonant('ㄸ'),
        'f' | 'F' => Jamo::Consonant('ㄹ'),
        'a' | 'A' => Jamo::Consonant('ㅁ'),
        'q' => Jamo::Consonant('ㅂ'),
        'Q' => Jamo::Consonant('ㅃ'),
        't' => Jamo::Consonant('ㅅ'),
        'T' => Jamo::Consonant('ㅆ'),
        'd' | 'D' => Jamo::Consonant('ㅇ'),
        'w' => Jamo::Consonant('ㅈ'),
        'W' => Jamo::Consonant('ㅉ'),
        'c' | 'C' => Jamo::Consonant('ㅊ'),
        'z' | 'Z' => Jamo::Consonant('ㅋ'),
        'x' | 'X' => Jamo::Consonant('ㅌ'),
        'v' | 'V' => Jamo::Consonant('ㅍ'),
        'g' | 'G' => Jamo::Consonant('ㅎ'),
        'k' | 'K' => Jamo::Vowel('ㅏ'),
        'o' => Jamo::Vowel('ㅐ'),
        'i' | 'I' => Jamo::Vowel('ㅑ'),
        'O' => Jamo::Vowel('ㅒ'),
        'j' | 'J' => Jamo::Vowel('ㅓ'),
        'p' => Jamo::Vowel('ㅔ'),
        'u' | 'U' => Jamo::Vowel('ㅕ'),
        'P' => Jamo::Vowel('ㅖ'),
        'h' | 'H' => Jamo::Vowel('ㅗ'),
        'y' | 'Y' => Jamo::Vowel('ㅛ'),
        'n' | 'N' => Jamo::Vowel('ㅜ'),
        'b' | 'B' => Jamo::Vowel('ㅠ'),
        'm' | 'M' => Jamo::Vowel('ㅡ'),
        'l' | 'L' => Jamo::Vowel('ㅣ'),
        _ => return None,
    };
    Some(jamo)
}
