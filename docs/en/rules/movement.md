# Movement

Units move during the Battle Planning Phase; units held in reserve move again in the Reserve Phase (see [Reserves](reserve.md)). Each move order takes a unit to one destination hex along the cheapest legal route. A unit may use only one movement system per Battle Planning Phase: Tactical, March, Rail, Air Transport, Paradrop, or Sea Transport. It may split that movement into several orders of the same system, as long as the total stays within its allowance.

## Movement allowance

- **Tactical movement:** the printed Movement Allowance. A unit that is Out of Movement Supply moves at half its printed allowance, rounded down.
- **March movement:** double the printed Movement Allowance.
- **Disrupted or Suppressed units** may only move one hex, by Minimum movement (see [Air power](air-power.md)).
- **Rail movement:** up to 20 hexes per Battle Planning Phase.

## Movement costs

Entering a hex costs Movement Points according to its primary terrain:

| Terrain | Cost |
| --- | --- |
| Clear | 1 |
| Forest | 1 |
| Marsh | 1 |
| Rough | 2 |
| Mountain | 3 |
| City (any size) | 1 |

A city replaces the terrain beneath it for movement. Crossing a Major River hexside costs 1 additional point. Crossing a Minor River hexside costs nothing extra.

## Prohibited terrain

Ground units may never:

- enter an All-Sea hex;
- cross an All-Sea hexside (a hexside that lies entirely in water), except where a Causeway crosses it;
- cross a Blocked hexside;
- enter a hex containing an enemy unit;
- enter an enemy Free City (see [Cities](cities.md)).

Only Tactical movement may enter an enemy Conquered City.

## Zones of control

A unit with an Attack Strength of 1 or more, and every HQ, exerts a Zone of Control (ZOC) into its own hex and the six adjacent hexes. A unit with an Attack Strength of 0 exerts a ZOC only in its own hex. Units under a train marker exert no ZOC. ZOCs extend across Blocked and All-Sea hexsides.

A hex in an enemy ZOC is an EZOC hex. With Tactical movement:

- entering an EZOC hex costs 1 additional point, and leaving one costs 1 additional point;
- entering a hex in an enemy Air Interdiction Zone costs 1 additional point;
- a Soft unit must stop in the first EZOC hex it enters and may not move again that phase;
- a Soft unit that starts in an EZOC hex may move directly into an adjacent EZOC hex only if a friendly unit or friendly Free City already occupies that hex, and must stop there;
- a Hard unit may continue moving through EZOC hexes as long as it can pay the costs.

## March movement

March movement must start in and stay within friendly Airspace (see [Air power](air-power.md)), and may not enter an enemy Air Interdiction Zone. It is also not available to a unit that:

- is an HQ;
- is Out of Movement Supply;
- starts in an EZOC hex.

A marching unit may never enter an EZOC hex or an enemy-controlled city.

## Minimum movement

A unit that has not yet moved this phase may always move one hex, even if it lacks the Movement Points to enter it. It still may not enter Prohibited Terrain or an enemy-occupied hex. A Soft unit still may not move from one EZOC hex into another unless a friendly unit or friendly Free City already occupies the destination. Minimum movement ends the unit's movement for the phase.

## The Danish Ferry

The Danish Ferry crosses the All-Sea hexside between hexes 1513 and 1514. Each Battle Planning Phase, one NATO unit may cross in each direction. The unit must not have moved yet that phase. The crossing is Minimum movement, so the unit stops on the other side. The Warsaw Pact cannot use the ferry.

## Rail movement

An Entrained unit may move by rail through land hexes. Each side may have only a limited number of steps Entrained at once: 8 for the Warsaw Pact and 10 for NATO. Units that are still Entraining do not count. At the start of its side's Battle Planning Phase, each Entraining unit becomes Entrained if the limit still allows; otherwise it keeps its Entraining marker. It may not cross Prohibited Terrain, start in or enter an EZOC hex, or enter an enemy-occupied hex, an enemy-controlled city, an enemy Air Interdiction Zone, or any hex that is not friendly Airspace. A unit may entrain only in friendly Airspace.

## Air, sea, and paradrop movement

Air Transport, Paradrop, and Sea Transport share these conditions. The unit must be in Movement Supply, not Disrupted, not under a train marker, and must not have moved yet that phase. It starts in a city (a port for Sea Transport) or in the Strategic Reserve; on the map, the starting hex must not be in enemy Airspace or in an EZOC (friendly units in the hex do not change this), and air movement may not start in Mountain terrain. Each Airlift or Sealift Command carries one step per turn, so a side may move as many steps per turn by air (Air Transport and Paradrop together) as it has Airlift Commands, and by sea as it has Sealift Commands. The scenario sets both numbers.

**Air Transport.** Airborne and Airmobile units fly from a city to a city their side controls. The flight may pass over enemy units and EZOCs but never through enemy Airspace. The destination must not be in enemy Airspace, in an EZOC, or in Mountain terrain. A unit in the Strategic Reserve takes off from one of its side's Reinforcement Sector entry hexes.

**Paradrop.** Units with the Airborne symbol only (not Airmobile units) may drop onto any Clear or Marsh hex, even in enemy Airspace or in an EZOC, and may attack adjacent enemies in the following Combat Phase. They may not land on an enemy unit or in an enemy-controlled city.

**Sea Transport.** Any unit may sail from a port to a port its side controls, along All-Sea, Coastal, and Major River hexes that are not in enemy Airspace; a river hex in an EZOC blocks the way. The destination must not be in enemy Airspace or in an EZOC. A unit in the Strategic Reserve sets sail from an All-Sea hex on its side's map edge.

Units in the Strategic Reserve leave it only by Air Transport, Paradrop, or Sea Transport; they may not entrain there.

## Stacking

At the end of every move, a hex may hold at most four steps of Maneuver units plus one HQ.
