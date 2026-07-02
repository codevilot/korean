#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderMode {
    VisibleTail,
    Delayed,
    DelayedPreview,
    Preedit,
    Safe,
}

impl RenderMode {
    pub(crate) fn from_env() -> Self {
        match std::env::var("KOREAN_RENDER_MODE").as_deref() {
            Ok("preedit") => Self::Preedit,
            Ok("safe") => Self::Safe,
            Ok("delayed") => Self::Delayed,
            Ok("delayed_preview") => Self::DelayedPreview,
            Ok("visible_tail") => Self::VisibleTail,
            Err(_) => Self::DelayedPreview,
            Ok(other) => {
                crate::debug_log(format_args!(
                    "unknown render_mode={other}, falling back to delayed_preview"
                ));
                Self::DelayedPreview
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeleteMode {
    Backspace,
    BackspacePair,
    Surrounding,
}

impl DeleteMode {
    pub(crate) fn from_env() -> Self {
        match std::env::var("KOREAN_DELETE_MODE").as_deref() {
            Ok("surrounding") | Err(_) => Self::Surrounding,
            Ok("backspace_press") => Self::Backspace,
            Ok("backspace_pair") => Self::BackspacePair,
            Ok(other) => {
                crate::debug_log(format_args!(
                    "unknown delete_mode={other}, falling back to surrounding"
                ));
                Self::Surrounding
            }
        }
    }
}
