//! City hex control (rule 30): Free and Conquered Cities.

use crate::engine::GameEngine;
use crate::event::GameEvent;
use crate::model::{CityControlChange, CityControlState, HexId, SideId};

impl GameEngine {
    /// Alliance that controlled the city at the start of play.
    pub(crate) fn city_owner(&self, hex_id: &HexId) -> Option<&SideId> {
        self.scenario
            .map
            .hexes
            .iter()
            .find(|hex| hex.id == *hex_id)
            .and_then(|hex| hex.city.as_ref())
            .map(|city| &city.owner)
    }

    /// Whether the hex is a Free City held by the other alliance.
    pub(crate) fn is_enemy_free_city(&self, side_id: &SideId, hex_id: &HexId) -> bool {
        self.city_control.get(hex_id).is_some_and(|controller| {
            controller != side_id && Some(controller) == self.city_owner(hex_id)
        })
    }

    pub(crate) fn city_states(&self) -> Vec<CityControlState> {
        self.city_control
            .iter()
            .filter_map(|(hex_id, controller)| {
                let owner = self.city_owner(hex_id)?.clone();
                Some(CityControlState {
                    hex_id: hex_id.clone(),
                    free: owner == *controller,
                    owner,
                    controller: controller.clone(),
                })
            })
            .collect()
    }

    /// 30.1.1, 30.3.2: a unit moving by Tactical movement takes control of every
    /// enemy-controlled Conquered City it enters, even without stopping there.
    /// Enemy Free Cities are never entered by movement, so they never change here.
    pub(crate) fn take_cities_along(
        &mut self,
        side_id: &SideId,
        path: &[HexId],
        events: &mut Vec<GameEvent>,
    ) -> Vec<CityControlChange> {
        let mut changes = Vec::new();
        for hex_id in path {
            let Some(controller) = self.city_control.get(hex_id).cloned() else {
                continue;
            };
            if controller == *side_id || self.is_enemy_free_city(side_id, hex_id) {
                continue;
            }
            self.city_control.insert(hex_id.clone(), side_id.clone());
            events.push(GameEvent::CityControlChanged {
                hex_id: hex_id.clone(),
                controller: side_id.clone(),
                free: self.city_owner(hex_id) == Some(side_id),
            });
            changes.push(CityControlChange {
                hex_id: hex_id.clone(),
                previous_controller: controller,
            });
        }
        changes
    }

    /// Reverts control changes made by an undone movement.
    pub(crate) fn restore_city_control(
        &mut self,
        changes: &[CityControlChange],
        events: &mut Vec<GameEvent>,
    ) {
        for change in changes.iter().rev() {
            self.city_control
                .insert(change.hex_id.clone(), change.previous_controller.clone());
            events.push(GameEvent::CityControlChanged {
                hex_id: change.hex_id.clone(),
                controller: change.previous_controller.clone(),
                free: self.city_owner(&change.hex_id) == Some(&change.previous_controller),
            });
        }
    }
}
