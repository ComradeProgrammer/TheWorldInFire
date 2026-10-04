use serde::{Deserialize, Serialize};

/// A stable, serializable rejection suitable for plugin and IPC responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleError {
    /// Machine-readable error code used by clients to select behavior.
    pub code: String,
    /// Human-readable description of the rejected operation.
    pub message: String,
}

impl RuleError {
    /// Constructs a rule error with an owned code and explanatory message.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// Constructs the error for a malformed message crossing the plugin boundary.
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::new("pluginProtocol", message)
    }
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuleError {}
