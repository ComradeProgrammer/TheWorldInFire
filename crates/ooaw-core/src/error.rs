use serde::{Deserialize, Serialize};

/// A stable, serializable rules-engine error suitable for IPC responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleError {
    /// Machine-readable error code used by clients to select behavior.
    pub code: String,
    /// Human-readable description of the rejected operation.
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
