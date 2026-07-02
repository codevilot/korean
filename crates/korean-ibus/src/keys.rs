pub(crate) const IBUS_SHIFT_MASK: u32 = 1 << 0;
#[cfg(test)]
pub(crate) const IBUS_LOCK_MASK: u32 = 1 << 1;
pub(crate) const IBUS_CONTROL_MASK: u32 = 1 << 2;
pub(crate) const IBUS_MOD1_MASK: u32 = 1 << 3;
pub(crate) const IBUS_RELEASE_MASK: u32 = 1 << 30;
pub(crate) const IBUS_CAP_SURROUNDING_TEXT: u32 = 1 << 5;

pub(crate) const KEY_BACKSPACE: u32 = 0xff08;
pub(crate) const KEYCODE_BACKSPACE: u32 = 14;
pub(crate) const KEYCODE_TAB: u32 = 15;
pub(crate) const KEYCODE_RETURN: u32 = 28;
pub(crate) const KEYCODE_LEFT: u32 = 105;
pub(crate) const KEYCODE_UP: u32 = 103;
pub(crate) const KEYCODE_RIGHT: u32 = 106;
pub(crate) const KEYCODE_DOWN: u32 = 108;
pub(crate) const KEY_RETURN: u32 = 0xff0d;
pub(crate) const KEY_ESCAPE: u32 = 0xff1b;
pub(crate) const KEY_CAPS_LOCK: u32 = 0xffe5;
pub(crate) const KEY_CONTROL_L: u32 = 0xffe3;
pub(crate) const KEY_CONTROL_R: u32 = 0xffe4;
pub(crate) const KEY_SHIFT_L: u32 = 0xffe1;
pub(crate) const KEY_SHIFT_R: u32 = 0xffe2;
pub(crate) const KEY_ALT_L: u32 = 0xffe9;
pub(crate) const KEY_ALT_R: u32 = 0xffea;
pub(crate) const KEY_SPACE: u32 = 0x020;
pub(crate) const KEY_TAB: u32 = 0xff09;
pub(crate) const KEY_LEFT: u32 = 0xff51;
pub(crate) const KEY_UP: u32 = 0xff52;
pub(crate) const KEY_RIGHT: u32 = 0xff53;
pub(crate) const KEY_DOWN: u32 = 0xff54;

pub(crate) fn ascii_letter(keyval: u32) -> Option<char> {
    char::from_u32(keyval).filter(|ch| ch.is_ascii_alphabetic())
}

pub(crate) fn ascii_symbol_or_digit(keyval: u32) -> Option<char> {
    let ch = char::from_u32(keyval)?;
    if ch.is_ascii_graphic() && !ch.is_ascii_alphabetic() {
        Some(ch)
    } else {
        None
    }
}

pub(crate) fn has_command_modifier(modifiers: u32) -> bool {
    modifiers & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK) != 0
}

pub(crate) fn is_modifier_key(keyval: u32) -> bool {
    matches!(
        keyval,
        KEY_SHIFT_L | KEY_SHIFT_R | KEY_CONTROL_L | KEY_CONTROL_R | KEY_ALT_L | KEY_ALT_R
    )
}

pub(crate) fn forward_keycode(keyval: u32) -> Option<u32> {
    match keyval {
        KEY_TAB => Some(KEYCODE_TAB),
        KEY_RETURN => Some(KEYCODE_RETURN),
        KEY_LEFT => Some(KEYCODE_LEFT),
        KEY_UP => Some(KEYCODE_UP),
        KEY_RIGHT => Some(KEYCODE_RIGHT),
        KEY_DOWN => Some(KEYCODE_DOWN),
        _ => None,
    }
}
