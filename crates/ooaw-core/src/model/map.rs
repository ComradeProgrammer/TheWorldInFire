use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::{HexId, SideId};

/// Complete rules and presentation data for one scenario map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapDefinition {
    /// Stable map identifier supplied to clients with the scenario snapshot.
    #[serde(default = "default_nato_map_id")]
    pub id: String,
    /// Version of the traced map-data format.
    pub version: u16,
    /// Human-readable provenance of the map data.
    pub source: String,
    /// Hex-grid geometry used by rendering and hit testing.
    pub grid: MapGrid,
    /// Rules-relevant terrain and feature data for every playable hex.
    pub hexes: Vec<MapHex>,
    /// Symbols drawn over the map.
    pub symbols: Vec<MapSymbol>,
    /// Filled water polygons used by the renderer.
    pub water: Vec<WaterArea>,
    /// Rivers, borders, coastlines, and other traced lines.
    pub lines: Vec<MapLine>,
    /// Named command-zone boundary lines.
    pub command_lines: Vec<CommandLine>,
    /// Decorative city-area outlines.
    pub cities: Vec<CityOutline>,
    /// Causeway geometry.
    pub causeways: Vec<Causeway>,
    /// Text labels drawn over the map.
    pub labels: Vec<MapLabel>,
    /// Rules-relevant features on specific hexsides.
    pub hexsides: Vec<MapHexside>,
}

/// Geometry of the printed pointy-top hex grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapGrid {
    /// Horizontal width of one hex in map pixels.
    pub hex_width: f64,
    /// Vertical distance between adjacent rows.
    pub row_spacing: f64,
    /// Horizontal origin of the grid.
    pub origin_x: f64,
    /// Vertical origin of the grid.
    pub origin_y: f64,
    /// Printed column number at the left edge of the grid.
    pub column_base: i16,
    /// Width of the traced board in map pixels.
    pub width: f64,
    /// Height of the traced board in map pixels.
    pub height: f64,
}

/// Natural terrain occupying a map hex.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Terrain {
    /// All-sea terrain.
    Sea,
    /// Clear terrain.
    Clear,
    /// Marsh terrain.
    Marsh,
    /// Forest terrain.
    Forest,
    /// Rough terrain.
    Rough,
    /// Mountain terrain.
    Mountain,
}

/// Victory and defense category of a city.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CityKind {
    /// Minor city.
    Minor,
    /// Major city.
    Major,
    /// Key city.
    Key,
}

/// Operational command zone printed on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandZone {
    /// Baltic Approaches command zone.
    #[serde(rename = "BALTAP")]
    Baltap,
    /// Northern Army Group command zone.
    #[serde(rename = "NORTHAG")]
    Northag,
    /// Central Army Group command zone.
    #[serde(rename = "CENTAG")]
    Centag,
}

/// City data attached to a hex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCity {
    /// Printed city name.
    pub name: String,
    /// City category.
    pub kind: CityKind,
    /// Printed organic defense strength.
    pub defense: u16,
    /// Alliance that controls the city at the start of play (rule 30.1).
    pub owner: SideId,
    /// Whether the city projects Airspace control (false for West Berlin, 11.5).
    #[serde(
        default = "contests_airspace_by_default",
        skip_serializing_if = "is_true"
    )]
    pub contests_airspace: bool,
    /// An isolated enclave (West Berlin) that supplies only units in or adjacent to it (10.3.3.4).
    #[serde(default, skip_serializing_if = "is_false")]
    pub enclave: bool,
}

/// Defaults omitted city airspace participation to true during deserialization.
fn contests_airspace_by_default() -> bool {
    true
}

/// Identifies the default true value so serialization can omit the city airspace flag.
fn is_true(value: &bool) -> bool {
    *value
}

/// Identifies the default false value so serialization can omit the enclave flag.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Rules and presentation data for one playable hex.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapHex {
    /// Printed RRCC hex identifier.
    pub id: HexId,
    /// Printed row number.
    pub row: u16,
    /// Printed column number.
    pub col: u16,
    /// Natural terrain underneath any city.
    pub terrain: Terrain,
    /// Whether the hex touches a coastline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coastal: Option<bool>,
    /// City in the hex, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<MapCity>,
    /// Printed port capacity, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Printed town name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub town: Option<String>,
    /// Whether the hex is a mobilization site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobilization: Option<bool>,
    /// Operational command zone containing the hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_zone: Option<CommandZone>,
}

/// One rules-relevant feature printed on a hexside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HexsideFeature {
    /// Boundary between NATO corps deployment areas.
    CorpsBoundary,
    /// Boundary between Warsaw Pact front deployment areas.
    FrontBoundary,
    /// Hexside that blocks movement or supply where the rules apply it.
    Blocked,
    /// Hexside drawn entirely over sea water; prohibited to ground movement.
    AllSea,
    /// Causeway that permits ground movement across an all-sea hexside.
    Causeway,
    /// Major river running along the hexside.
    MajorRiver,
    /// Minor river running along the hexside.
    MinorRiver,
    /// The Danish Ferry crossing between hexes 1513 and 1514.
    DanishFerry,
}

/// Features shared by two adjacent hexes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapHexside {
    /// First adjacent hex.
    pub a: HexId,
    /// Second adjacent hex.
    pub b: HexId,
    /// Features present on the shared side.
    pub features: Vec<HexsideFeature>,
}

/// Kind of a traced line drawn on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    /// Major river.
    MajorRiver,
    /// Minor river.
    MinorRiver,
    /// National boundary.
    NationalBoundary,
    /// Iron Curtain boundary.
    IronCurtain,
    /// Coastline.
    Coast,
    /// Boundary between sea areas.
    SeaBoundary,
}

/// Traced polyline in map-pixel coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLine {
    /// Semantic kind of the line.
    pub kind: LineKind,
    /// Flat x/y coordinate sequence.
    pub points: Vec<f64>,
}

/// Filled water polygon in map-pixel coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterArea {
    /// Flat x/y coordinate sequence for the exterior polygon.
    pub outer: Vec<f64>,
    /// Optional flat x/y coordinate sequences for interior holes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holes: Option<Vec<Vec<f64>>>,
}

/// Named boundary between two command zones.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLine {
    /// Display name of the boundary.
    pub name: String,
    /// Command zone north of the boundary.
    pub north: CommandZone,
    /// Command zone south of the boundary.
    pub south: CommandZone,
    /// Flat x/y coordinate sequence.
    pub points: Vec<f64>,
}

/// Decorative outline associated with a city category.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CityOutline {
    /// City category represented by the outline.
    pub kind: CityKind,
    /// Flat x/y coordinate sequence.
    pub points: Vec<f64>,
}

/// Traced causeway geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Causeway {
    /// Flat x/y coordinate sequence.
    pub points: Vec<f64>,
    /// Rendered width in map pixels.
    pub width: f64,
}

/// Symbol drawn over the map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MapSymbol {
    /// Organic defense-strength symbol.
    Defense {
        /// Hex containing the symbol.
        hex: HexId,
        /// Printed defense value.
        value: u16,
        /// Horizontal map-pixel position.
        x: f64,
        /// Vertical map-pixel position.
        y: f64,
    },
    /// Port-capacity symbol.
    Port {
        /// Hex containing the symbol.
        hex: HexId,
        /// Printed port capacity.
        value: u16,
        /// Horizontal map-pixel position.
        x: f64,
        /// Vertical map-pixel position.
        y: f64,
    },
    /// Mobilization-site symbol.
    Mobilization {
        /// Hex containing the symbol.
        hex: HexId,
        /// Horizontal map-pixel position.
        x: f64,
        /// Vertical map-pixel position.
        y: f64,
    },
}

/// Category of a text label drawn over the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LabelKind {
    /// Sea-area label.
    Sea,
    /// Island label.
    Island,
    /// River label.
    River,
    /// Command-zone label.
    Command,
    /// NATO deployment label.
    Deployment,
    /// Warsaw Pact deployment label.
    DeploymentPact,
    /// Key-city label.
    KeyCity,
    /// Major-city label.
    MajorCity,
    /// Minor-city label.
    MinorCity,
    /// Town label.
    Town,
}

/// Text label drawn at a map-pixel location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLabel {
    /// Displayed text.
    pub text: String,
    /// Label category controlling presentation.
    pub kind: LabelKind,
    /// Horizontal map-pixel position.
    pub x: f64,
    /// Vertical map-pixel position.
    pub y: f64,
}

/// Provides the NATO map identifier when deserializing data without an explicit map ID.
fn default_nato_map_id() -> String {
    "nato-central-europe".to_owned()
}

/// Returns the NATO map embedded in the rules crate and cloned into a scenario.
pub(crate) fn nato_map() -> MapDefinition {
    static MAP: OnceLock<MapDefinition> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/natoMap.json"))
            .expect("embedded NATO map data must be valid")
    })
    .clone()
}
