//! The kernel's authoritative mutable state and the changes plugins make to it.

use std::collections::{BTreeMap, HashSet};

use ooaw_plugin_api::protocol::{Change, RESERVED_STATE_KEYS};
use ooaw_plugin_api::serde_json::{self, Value};
use ooaw_plugin_api::{
    GameState, GameStatus, HexId, PendingDecision, RuleError, SideId, TurnPosition, Unit, UnitId,
    UnitLocation,
};

/// Mutable state of one game.
///
/// Rules plugins change it only through [`Change`]s, which the kernel checks
/// for structural consistency (units on known hexes, valid step indexes,
/// known cities) but not against any game rule.
#[derive(Clone, Debug, PartialEq)]
pub struct KernelState {
    /// One-based game-turn number.
    pub game_turn: u16,
    /// Zero-based position in the scenario's repeated turn sequence.
    pub step_index: usize,
    /// Whether the game still accepts commands.
    pub status: GameStatus,
    /// Units in play, keyed by identifier.
    pub units: BTreeMap<UnitId, Unit>,
    /// Current controller of every city hex.
    pub city_control: BTreeMap<HexId, SideId>,
    /// Player decision required before play continues.
    pub pending_decision: Option<PendingDecision>,
    /// Rules-owned top-level state keyed by name.
    pub rules: BTreeMap<String, Value>,
}

impl KernelState {
    /// Builds the opening state from a plugin-supplied scenario state.
    pub(crate) fn opening(state: GameState) -> Self {
        Self {
            game_turn: 1,
            step_index: 0,
            status: GameStatus::InProgress,
            units: state
                .units
                .into_iter()
                .map(|unit| (unit.id.clone(), unit))
                .collect(),
            city_control: state.city_control,
            pending_decision: state.pending_decision,
            rules: state.rules,
        }
    }

    /// Current turn position.
    pub(crate) fn turn(&self) -> TurnPosition {
        TurnPosition {
            game_turn: self.game_turn,
            step_index: self.step_index,
            status: self.status,
        }
    }

    /// The part of the state a plugin mirrors.
    pub(crate) fn mirror(&self) -> GameState {
        GameState {
            units: self.units.values().cloned().collect(),
            city_control: self.city_control.clone(),
            pending_decision: self.pending_decision.clone(),
            rules: self.rules.clone(),
        }
    }

    /// Applies one plugin change after checking it against the map.
    pub(crate) fn apply(
        &mut self,
        change: Change,
        hexes: &HashSet<HexId>,
    ) -> Result<(), RuleError> {
        match change {
            Change::PutUnit { unit } => {
                check_unit(&unit, hexes)?;
                self.units.insert(unit.id.clone(), unit);
            }
            Change::UpdateUnit { unit_id, fields } => {
                let unit = self.units.get(&unit_id).ok_or_else(|| {
                    violation(format!("Cannot update unknown unit {}", unit_id.0))
                })?;
                let Value::Object(mut object) =
                    serde_json::to_value(unit).map_err(|error| violation(error.to_string()))?
                else {
                    return Err(violation("A unit must serialize to an object"));
                };
                object.extend(fields);
                let updated: Unit =
                    serde_json::from_value(Value::Object(object)).map_err(|error| {
                        violation(format!(
                            "Update leaves unit {} malformed: {error}",
                            unit_id.0
                        ))
                    })?;
                if updated.id != unit_id {
                    return Err(violation(format!(
                        "Unit {} may not change its ID",
                        unit_id.0
                    )));
                }
                check_unit(&updated, hexes)?;
                self.units.insert(unit_id, updated);
            }
            Change::RemoveUnit { unit_id } => {
                if self.units.remove(&unit_id).is_none() {
                    return Err(violation(format!(
                        "Cannot remove unknown unit {}",
                        unit_id.0
                    )));
                }
            }
            Change::SetCityControl { hex_id, side_id } => {
                let Some(controller) = self.city_control.get_mut(&hex_id) else {
                    return Err(violation(format!("Hex {} holds no city", hex_id.0)));
                };
                *controller = side_id;
            }
            Change::SetRules { key, value } => {
                if RESERVED_STATE_KEYS.contains(&key.as_str()) {
                    return Err(violation(format!("Rules state may not use the name {key}")));
                }
                self.rules.insert(key, value);
            }
            Change::SetPendingDecision { decision } => self.pending_decision = decision,
        }
        Ok(())
    }
}

/// Checks that a unit's step index and location are valid.
pub(crate) fn check_unit(unit: &Unit, hexes: &HashSet<HexId>) -> Result<(), RuleError> {
    if unit.steps.is_empty() || unit.strength_step_index >= unit.steps.len() {
        return Err(violation(format!(
            "Unit {} has an invalid strength-step setup",
            unit.id.0
        )));
    }
    if let UnitLocation::Hex { hex_id } = &unit.location {
        if !hexes.contains(hex_id) {
            return Err(violation(format!(
                "Unit {} references an unknown map hex",
                unit.id.0
            )));
        }
    }
    Ok(())
}

/// The error for a plugin change that would leave the state inconsistent.
pub(crate) fn violation(message: impl Into<String>) -> RuleError {
    RuleError::new("pluginStateViolation", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ooaw_plugin_api::serde_json::json;

    fn state() -> (KernelState, HashSet<HexId>) {
        let unit: Unit = serde_json::from_value(json!({
            "id": "a", "name": "A", "sideId": "nato", "nationId": "westGermany",
            "unitTypeId": "armoredBrigade", "formationId": null, "traits": [],
            "steps": [{ "attack": 3 }, { "attack": 1 }], "strengthStepIndex": 0,
            "location": { "type": "hex", "hexId": "0101" },
            "supply": "supplied", "mod.morale": 3,
        }))
        .unwrap();
        let mut game = KernelState::opening(GameState::default());
        game.units.insert(unit.id.clone(), unit);
        game.city_control
            .insert(HexId("0102".to_owned()), SideId("nato".to_owned()));
        let hexes = ["0101", "0102"]
            .into_iter()
            .map(|id| HexId(id.to_owned()))
            .collect();
        (game, hexes)
    }

    fn update(fields: Value) -> Change {
        Change::UpdateUnit {
            unit_id: UnitId("a".to_owned()),
            fields: fields.as_object().unwrap().clone(),
        }
    }

    #[test]
    fn an_update_keeps_markers_it_does_not_name() {
        let (mut game, hexes) = state();
        game.apply(
            update(json!({ "supply": "outOfSupply", "strengthStepIndex": 1 })),
            &hexes,
        )
        .unwrap();
        let unit = &game.units[&UnitId("a".to_owned())];
        assert_eq!(unit.strength_step_index, 1);
        assert_eq!(unit.markers["supply"], json!("outOfSupply"));
        assert_eq!(unit.markers["mod.morale"], json!(3));
    }

    #[test]
    fn inconsistent_changes_are_refused() {
        let (mut game, hexes) = state();
        let refused = [
            update(json!({ "location": { "type": "hex", "hexId": "9999" } })),
            update(json!({ "strengthStepIndex": 2 })),
            update(json!({ "id": "b" })),
            Change::RemoveUnit {
                unit_id: UnitId("b".to_owned()),
            },
            Change::SetCityControl {
                hex_id: HexId("0101".to_owned()),
                side_id: SideId("nato".to_owned()),
            },
            Change::SetRules {
                key: "units".to_owned(),
                value: json!([]),
            },
        ];
        for change in refused {
            let error = game.apply(change.clone(), &hexes).unwrap_err();
            assert_eq!(error.code, "pluginStateViolation", "{change:?}");
        }
    }
}
