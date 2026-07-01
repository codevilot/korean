#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    En,
    Ko,
    EnCaps,
}

#[derive(Clone, Debug)]
pub struct ModeState {
    mode: InputMode,
}

impl Default for ModeState {
    fn default() -> Self {
        Self {
            mode: InputMode::Ko,
        }
    }
}

impl ModeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }

    pub fn caps_tap(&mut self) -> InputMode {
        self.mode = match self.mode {
            InputMode::En => InputMode::Ko,
            InputMode::Ko => InputMode::En,
            InputMode::EnCaps => InputMode::Ko,
        };
        self.mode
    }

    pub fn caps_hold(&mut self) -> InputMode {
        self.mode = match self.mode {
            InputMode::En => InputMode::EnCaps,
            InputMode::EnCaps => InputMode::En,
            InputMode::Ko => InputMode::EnCaps,
        };
        self.mode
    }

    pub fn shift_caps(&mut self) -> InputMode {
        self.caps_hold()
    }
}

#[cfg(test)]
mod tests {
    use super::{InputMode, ModeState};

    #[test]
    fn caps_tap_transitions() {
        let mut state = ModeState::new();
        assert_eq!(state.caps_tap(), InputMode::En);
        assert_eq!(state.caps_tap(), InputMode::Ko);
        state.set_mode(InputMode::EnCaps);
        assert_eq!(state.caps_tap(), InputMode::Ko);
    }

    #[test]
    fn caps_hold_transitions() {
        let mut state = ModeState::new();
        assert_eq!(state.caps_hold(), InputMode::EnCaps);
        state.set_mode(InputMode::En);
        assert_eq!(state.caps_hold(), InputMode::EnCaps);
        assert_eq!(state.caps_hold(), InputMode::En);
        state.set_mode(InputMode::Ko);
        assert_eq!(state.shift_caps(), InputMode::EnCaps);
    }
}
