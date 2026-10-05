//! Example third-party rules plugin: night fighting.
//!
//! Every battle fought under the official NATO rules shifts one column in the
//! defender's favour. The plugin owns no phases, commands, or state; it takes
//! part in one filter of the official rules, `nato.combat.columnShifts`, and
//! appends a shift to the list it receives.
//!
//! It deliberately uses plain JSON rather than the `ooaw-nato` crate's types,
//! to show the contract a plugin in any language would follow.

use ooaw_plugin_sdk::api::protocol::{Change, PluginManifest, ABI_VERSION};
use ooaw_plugin_sdk::api::serde_json::{json, Value};
use ooaw_plugin_sdk::api::{RuleError, TurnPosition};
use ooaw_plugin_sdk::{export_plugin, Attachment, RulesPlugin};

/// Plugin identifier.
pub const PLUGIN_ID: &str = "example.nightFighting";

/// The official rules' column-shift filter.
const COLUMN_SHIFTS: &str = "nato.combat.columnShifts";

/// The night-fighting plugin.
#[derive(Default)]
pub struct NightFighting;

impl RulesPlugin for NightFighting {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: PLUGIN_ID.to_owned(),
            name: "Night fighting (example)".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            abi_version: ABI_VERSION,
            stateless: true,
            phases: Vec::new(),
            commands: Vec::new(),
            queries: Vec::new(),
            filters: vec![COLUMN_SHIFTS.to_owned()],
            scenarios: Vec::new(),
        }
    }

    fn attach(&mut self, _attachment: Attachment) -> Result<(), RuleError> {
        Ok(())
    }

    fn set_turn(&mut self, _turn: TurnPosition) {}

    fn sync(&mut self, _state: Value) -> Result<(), RuleError> {
        Ok(())
    }

    fn take_changes(&self) -> Result<Vec<Change>, RuleError> {
        Ok(Vec::new())
    }

    fn filter(&self, name: &str, _input: Value, value: Value) -> Result<Value, RuleError> {
        if name != COLUMN_SHIFTS {
            return Ok(value);
        }
        let Value::Array(mut shifts) = value else {
            return Err(RuleError::new(
                "invalidFilterValue",
                "Column shifts must be a list",
            ));
        };
        shifts.push(json!({ "reason": "example.nightFighting", "shift": -1 }));
        Ok(Value::Array(shifts))
    }
}

export_plugin!(NightFighting);
