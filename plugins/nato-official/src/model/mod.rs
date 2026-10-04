mod combat;
mod nato_map;
mod planning;
mod rules;
mod scenario;
mod scenario_baltap;
mod scenario_campaign;
mod strike;
mod unit;

pub use combat::{
    BattleOdds, BattleReport, ColumnShift, ColumnShiftReason, CombatObjective, CombatOptions,
    CombatResult, CombatState, CounterattackRoll, PendingAdvance, StrengthModifier, UnitStrength,
    ODDS_COLUMNS,
};
pub use ooaw_plugin_api::{
    Causeway, CityControlState, CityKind, CityOutline, CommandLine, CommandZone, FormationId,
    HexId, HexsideFeature, LabelKind, LineKind, MapCity, MapDefinition, MapGrid, MapHex,
    MapHexside, MapLabel, MapLine, MapSymbol, NationId, PhaseActor, PhaseDefinition,
    PhaseExecution, PhaseId, ReinforcementSector, ScenarioSummary, SideDefinition, SideId, StepId,
    Terrain, UnitId, UnitLocation, UnitTraitId, UnitTypeId, WaterArea,
};
pub(crate) use planning::{has_incompatible_movement, movement_spent};
pub use planning::{
    BattlePlan, CityControlChange, MovementModeOptions, MovementOption, PlannedMovement,
    ReserveOption, ReserveState,
};
pub use rules::{BattlePlanningRules, MovementMode, TrainStatus};
pub use scenario::{
    find_scenario, list_scenarios, OffensiveSupportHq, ReinforcementDefinition, ScenarioDefinition,
    Withdrawal,
};
pub use strike::{
    AirInterdictionZone, AirMission, AirMissionKind, AirPointKind, AirPointSource, AirPoints,
    AirPowerRules, AirStrikeOptions, Airspace, Disruption, SideAirPower, StrikePlan,
    StrikeResolution, StrikeResult, StrikeTargetHex, StrikeTargetUnit,
};
pub use unit::{SupplyStatus, UnitDefinition, UnitState, UnitStepDefinition, UnitSupplyState};
