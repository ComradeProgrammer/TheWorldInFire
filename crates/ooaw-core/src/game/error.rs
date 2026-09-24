use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleError {
    pub code: String,
    pub message: String,
}

impl RuleError {
    pub(super) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    pub(super) fn game_complete() -> Self {
        Self::new("gameComplete", "The game has already completed")
    }
}
