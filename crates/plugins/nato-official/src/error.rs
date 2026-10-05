pub use ooaw_plugin_api::RuleError;

/// Constructs the standard error for actions attempted after the scenario has ended.
pub(crate) fn game_complete() -> RuleError {
    RuleError::new("gameComplete", "The game has already completed")
}
