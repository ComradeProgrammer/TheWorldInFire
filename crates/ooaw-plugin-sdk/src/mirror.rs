//! Helpers that turn differences between a plugin's baseline and its current
//! mirror into [`Change`]s.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::api::protocol::Change;
use crate::api::serde_json::{self, Map, Value};
use crate::api::{HexId, PendingDecision, RuleError, SideId, Unit, UnitId};

fn encode<T: Serialize>(value: &T) -> Result<Value, RuleError> {
    serde_json::to_value(value).map_err(|error| RuleError::protocol(error.to_string()))
}

fn object<T: Serialize>(value: &T) -> Result<Map<String, Value>, RuleError> {
    match encode(value)? {
        Value::Object(map) => Ok(map),
        _ => Err(RuleError::protocol(
            "A unit must serialize to a JSON object",
        )),
    }
}

/// Changes that turn `base` units into `current` units.
///
/// `T` is the plugin's own unit type, which must serialize to the shape of
/// [`Unit`]. Changed units are reported field by field, so markers that other
/// plugins own survive. A field the plugin no longer serializes is set to `null`.
pub fn unit_changes<T: Serialize + PartialEq>(
    base: &BTreeMap<UnitId, T>,
    current: &BTreeMap<UnitId, T>,
) -> Result<Vec<Change>, RuleError> {
    let mut changes = Vec::new();
    for (unit_id, unit) in current {
        match base.get(unit_id) {
            None => {
                let unit: Unit = serde_json::from_value(encode(unit)?).map_err(|error| {
                    RuleError::protocol(format!("Unit {} is malformed: {error}", unit_id.0))
                })?;
                changes.push(Change::PutUnit { unit });
            }
            Some(previous) if previous != unit => {
                let before = object(previous)?;
                let after = object(unit)?;
                let mut fields = Map::new();
                for (key, value) in &after {
                    if before.get(key) != Some(value) {
                        fields.insert(key.clone(), value.clone());
                    }
                }
                for key in before.keys() {
                    if !after.contains_key(key) {
                        fields.insert(key.clone(), Value::Null);
                    }
                }
                if !fields.is_empty() {
                    changes.push(Change::UpdateUnit {
                        unit_id: unit_id.clone(),
                        fields,
                    });
                }
            }
            Some(_) => {}
        }
    }
    for unit_id in base.keys() {
        if !current.contains_key(unit_id) {
            changes.push(Change::RemoveUnit {
                unit_id: unit_id.clone(),
            });
        }
    }
    Ok(changes)
}

/// Changes that turn `base` city control into `current` city control.
pub fn city_changes(
    base: &BTreeMap<HexId, SideId>,
    current: &BTreeMap<HexId, SideId>,
) -> Vec<Change> {
    current
        .iter()
        .filter(|(hex_id, side_id)| base.get(*hex_id) != Some(*side_id))
        .map(|(hex_id, side_id)| Change::SetCityControl {
            hex_id: hex_id.clone(),
            side_id: side_id.clone(),
        })
        .collect()
}

/// Changes that turn `base` rules-state entries into `current` ones.
pub fn rules_changes(
    base: &BTreeMap<String, Value>,
    current: &BTreeMap<String, Value>,
) -> Vec<Change> {
    current
        .iter()
        .filter(|(key, value)| base.get(*key) != Some(*value))
        .map(|(key, value)| Change::SetRules {
            key: key.clone(),
            value: value.clone(),
        })
        .collect()
}

/// The change, if any, that turns `base` into `current` pending decision.
pub fn decision_change(
    base: &Option<PendingDecision>,
    current: &Option<PendingDecision>,
) -> Option<Change> {
    (base != current).then(|| Change::SetPendingDecision {
        decision: current.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::serde_json::json;

    fn unit(id: &str, hex: &str, supply: &str) -> Value {
        json!({
            "id": id, "name": id, "sideId": "nato", "nationId": "westGermany",
            "unitTypeId": "armoredBrigade", "formationId": null, "traits": [],
            "steps": [{ "attack": 3 }], "strengthStepIndex": 0,
            "location": { "type": "hex", "hexId": hex },
            "supply": supply,
        })
    }

    fn units(entries: &[(&str, &str, &str)]) -> BTreeMap<UnitId, Value> {
        entries
            .iter()
            .map(|(id, hex, supply)| (UnitId((*id).to_owned()), unit(id, hex, supply)))
            .collect()
    }

    #[test]
    fn unit_changes_report_only_changed_fields() {
        let base = units(&[("a", "0101", "supplied"), ("b", "0102", "supplied")]);
        let current = units(&[("a", "0103", "supplied"), ("c", "0104", "supplied")]);
        let changes = unit_changes(&base, &current).unwrap();
        assert_eq!(changes.len(), 3);
        assert!(changes.contains(&Change::UpdateUnit {
            unit_id: UnitId("a".to_owned()),
            fields: json!({ "location": { "type": "hex", "hexId": "0103" } })
                .as_object()
                .unwrap()
                .clone(),
        }));
        assert!(changes
            .iter()
            .any(|change| matches!(change, Change::PutUnit { unit } if unit.id.0 == "c")));
        assert!(changes.contains(&Change::RemoveUnit {
            unit_id: UnitId("b".to_owned()),
        }));
        assert!(unit_changes(&current, &current).unwrap().is_empty());
    }

    #[test]
    fn rules_and_city_changes_cover_only_differences() {
        let base: BTreeMap<String, Value> =
            [("a".to_owned(), json!(1)), ("b".to_owned(), json!(null))].into();
        let current: BTreeMap<String, Value> =
            [("a".to_owned(), json!(1)), ("b".to_owned(), json!([2]))].into();
        assert_eq!(
            rules_changes(&base, &current),
            vec![Change::SetRules {
                key: "b".to_owned(),
                value: json!([2]),
            }]
        );
        let hex = HexId("0101".to_owned());
        let base = BTreeMap::from([(hex.clone(), SideId("nato".to_owned()))]);
        let current = BTreeMap::from([(hex.clone(), SideId("warsawPact".to_owned()))]);
        assert_eq!(city_changes(&base, &base), Vec::new());
        assert_eq!(city_changes(&base, &current).len(), 1);
    }
}
