"""Import campaign content from local references; never used by builds or tests.

Run from the repository root with Python and pypdf installed. Inputs under
internet/ remain read-only. Counter values come from the living Play Booklet's
order of battle; deployments and reinforcement-card positions come from VASSAL
2.4.1. Only structured game data is emitted (no images or VASSAL code).
"""

import copy
import json
import logging
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile

from pypdf import PdfReader

ROOT = Path(__file__).resolve().parents[1]
REFERENCES = ROOT / "internet"
MODULE = REFERENCES / "NATO_PZG_v2_4_1"
OUTPUT = ROOT / "crates/ooaw-core/data/natoCampaigns.json"
NATIONS = {
    "SO": "sovietUnion", "EG": "eastGermany", "CZ": "czechoslovakia",
    "PO": "poland", "BR": "unitedKingdom", "US": "unitedStates",
    "CA": "canada", "WG": "westGermany", "DA": "denmark",
    "NE": "netherlands", "BE": "belgium", "FR": "france",
}
NATION_NAMES = dict(zip(NATIONS, [
    "Soviet", "East German", "Czechoslovak", "Polish", "British", "U.S.",
    "Canadian", "West German", "Danish", "Dutch", "Belgian", "French",
]))
HEADINGS = dict(zip(
    ["UNITED KINGDOM", "UNITED STATES", "CANADA", "WEST GERMANY", "DENMARK",
     "NETHERLANDS", "BELGIUM", "FRANCE"],
    ["BR", "US", "CA", "WG", "DA", "NE", "BE", "FR"],
))
VALUE = r"\(?\d+\)?-\[?\(?\d+\)?\]?-\d+A?"
ROW = re.compile(
    rf"^(.*?)\s+({VALUE})\s+(?:(Blank|Reforger|{VALUE})\s+)?"
    r"(.*?)\s+(XXXXX|XXXX|XXX|XX|III|X)\s*(.*)$"
)
LEGEND = re.compile(r"\s+(NEGF|SEGF|CZF|POF|BAF|BEF|CAF|BC|Terr|BR I|US III|"
                    r"US VII|US V|CEN|ARNG|WG III|WG II|WG I|LJ|LZ|NE I|BE I|FR I|FAR)$")


def normalize(text):
    return re.sub(r"[^a-z0-9/]", "", text.lower())


def read_catalog():
    logging.getLogger("pypdf").setLevel(logging.ERROR)
    text = "\n".join(p.extract_text() for p in PdfReader(
        REFERENCES / "NATO_Play_Booklet_Updated_1-1-26.pdf").pages)
    sections = {
        "WP": text.split("[44.1] WP 1983 ORDER OF BATTLE")[-1].split("[44.2]")[0],
        "83": text.split("[44.3] NATO 1983 ORDER OF BATTLE")[-1].split("[44.4]")[0],
        "88": text.split("[44.4] NATO 1988 ORDER OF BATTLE")[-1].split("[45.0]")[0],
    }
    catalogs = {}
    for key, section in sections.items():
        rows = []
        nation = None
        for line in section.splitlines():
            line = line.strip()
            if line in HEADINGS:
                nation = HEADINGS[line]
            match = ROW.match(line)
            if not match:
                continue
            designation, front, back, kind, size, entry = match.groups()
            legend = LEGEND.search(designation)
            affiliation = legend[1] if legend else ""
            if legend:
                designation = designation[:legend.start()]
            designation = designation.removesuffix(" BALT")
            owner = nation
            if key == "WP":
                owner = ("EG" if designation.startswith(("E", "Gr")) else
                         "CZ" if designation.startswith("C") and designation not in ["CAF", "CZF"] else
                         "PO" if designation.startswith("P") and designation != "POF" else "SO")
            rows.append(dict(designation=designation, legend=affiliation, nation=owner,
                             front=front, back=back, kind=kind, size=size, entry=entry))
        catalogs[key] = rows
    catalogs["WP88"] = copy.deepcopy(catalogs["WP"])
    for row in catalogs["WP88"]:
        if row["designation"] == "6G (Division)":
            row.update(designation="90GT", front="10-[6]-6", back="5-[4]-6", kind="Armor")
        elif row["designation"] == "90GT":
            row.update(designation="6G (Division)", front="8-[6]-5", back="4-[4]-5", kind="Mech")
    # The VASSAL module and the scenario text (37.3.1/40.3.1) include this
    # 1988-only brigade, omitted from the booklet's abbreviated WP 1988 OB.
    catalogs["WP88"].append(dict(designation="83", legend="NEGF", nation="SO",
                                front="1-1-5A", back="Blank", kind="Airborne Infantry",
                                size="X", entry="A"))
    return catalogs


def read_pieces(setup, slots):
    encoded = zipfile.ZipFile(MODULE / f"{setup}_2.4_1.vsav").read("savedGame")
    assert encoded.startswith(b"!VCSK")
    data = bytes.fromhex(encoded[5:].decode())
    decoded = bytes(byte ^ data[0] for byte in data[1:]).decode("utf-8")
    for record in decoded.split("\x1b"):
        if not record.startswith("+/"):
            continue
        parts = re.split(r"(?<!\\)/", record)
        if len(parts) != 4 or parts[2] == "stack":
            continue
        state = re.split(r"\\*\t", parts[3])
        location = state[-1].split(";")
        slot = slots.get(location[3]) if len(location) >= 4 else None
        if slot is None:
            continue
        name = slot.get("entryName")
        if name.split()[0] not in NATIONS or " AH " in f" {name} ":
            continue
        traits = list(zip(re.split(r"\\*\t", parts[2]), state))
        yield dict(name=name, traits=traits, map=location[0], x=int(location[1]),
                   y=int(location[2]), gpid=location[3])


def counter_image(piece, year):
    images = []
    for definition, _ in piece["traits"]:
        if definition.startswith(("piece;", "emb2;")):
            fields = definition.split(";")
            if definition.startswith("emb2;") and len(fields) < 17:
                continue
            field = fields[3] if definition.startswith("piece;") else fields[16]
            images.extend(field.split(","))
    images = [s for s in images if s.endswith(".png") and
              not s.startswith(("Layer", "Markers", "Blank", "Reforger"))]
    images = [s for s in images if not (s.endswith("b.png") and s[:-5]+".png" in images)]
    specific = [s for s in images if str(year)[-2:] in s.split("_")[0]]
    common = [s for s in images if not re.search(r"83|88", s.split("_")[0])]
    chosen = specific or common
    assert chosen, (piece["name"], images)
    return chosen[0]


def identify(piece, image, catalog):
    nation = piece["name"].split()[0]
    stem = re.split(r"[_-]", image[:-4], maxsplit=1)[1]
    chunks = stem.split("-")
    token = re.sub(r" ?(Airborne|Arty|AT)$", "", chunks[-1])
    if nation in ["SO", "EG", "CZ", "PO"]:
        if token == "HQ":
            designation = chunks[-2]
        elif chunks[0] == "Ter":
            designation = token
        elif len(chunks) >= 3 and chunks[-2] not in ["BC"]:
            designation = token + "/" + chunks[-2].replace("28G", "28")
        elif chunks[0] in ["7G", "76G", "5G"]:
            designation = token.removesuffix("G") + "/" + chunks[0] if chunks[0] != "5G" else token + "/5G"
        else:
            designation = token
        if image == "CZ_CZF-8T-C13.png":
            designation = "C13T/8T"
        if image == "PO_POF-P4-P10.png":
            designation = "P10T/P4"
        if piece["name"] == "SO 6GMD":
            designation = "6G (Division)"
        if piece["name"] == "SO 6GMB":
            designation = "6G (Brigade)"
    else:
        if token == "HQ":
            designation = chunks[0] if nation in ["WG", "DA"] and chunks[0] in ["LJ", "LZ"] else nation + " " + chunks[0].replace("BRI", "I").replace("NEI", "I")
            if nation == "BE":
                designation = "BE I"
            if nation == "FR" and chunks[0] == "FAR":
                designation = "FAR"
        elif stem == "Berlin":
            designation = "Ber"
        elif len(chunks) >= 3 or (nation == "BE" and chunks[0] != "Ter" and len(chunks) == 2):
            formation = chunks[-2].replace("PzG", "PG").replace("PZG", "PG").replace("Pz", "PZ")
            if nation == "WG" and token == "51Hsb":
                formation = "6"
            if nation == "WG" and formation == "6PG":
                formation = "6"
            designation = token + "/" + formation
        else:
            designation = token
        designation = {"236/5": "256/5", "194": "194A"}.get(designation, designation)
        if nation == "BE" and designation.endswith("CRC"):
            designation = "CRC"
        if nation == "BR" and designation == "5 Para":
            designation = "5Para"
    matches = [r for r in catalog if r["nation"] == nation and
               normalize(r["designation"]) == normalize(designation)]
    assert len(matches) == 1, (piece["name"], image, designation, matches)
    return matches[0]


def unit_data(row):
    nation = NATIONS[row["nation"]]
    traits = []
    kind = row["kind"]
    size = {"III": "Regiment", "X": "Brigade", "XX": "Division"}.get(row["size"], "Headquarters")
    if "HQ" in kind:
        unit_type = "headquarters"
        traits.append("headquarters")
    elif "Airborne" in kind:
        unit_type = "airborne" + size
        traits.append("airborne")
    elif "Airmobile" in kind:
        unit_type = "airmobile" + size
        traits.append("airmobile")
    elif "Marine" in kind:
        unit_type = "marine" + size
        traits.append("marine")
    elif "Artillery" in kind:
        unit_type = "artillery" + size
        traits.append("artillery")
    elif "Cav" in kind:
        unit_type = "armoredCavalry" + size
    elif kind == "Armor":
        unit_type = "tank" + size
    elif kind == "Mech":
        unit_type = "mechanized" + size
    elif "Mountain" in kind:
        unit_type = "mountainInfantry" + size
    else:
        unit_type = "infantry" + size
    if row["legend"] == "Terr":
        traits.append("territorial")
    if "Marine" in kind and "marine" not in traits:
        traits.append("marine")
    if "[" in row["front"]:
        traits.append("hard")
    steps = [list(map(int, re.findall(r"\d+", row["front"])))]
    if row["back"] and re.fullmatch(VALUE, row["back"]):
        steps.append(list(map(int, re.findall(r"\d+", row["back"]))))
    ident = normalize(row["designation"]).replace("/", ".")
    label = kind if "HQ" in kind else f"{kind} {size}"
    return dict(id=nation + "." + ident, name=f'{NATION_NAMES[row["nation"]]} {row["designation"]} {label}',
                nationId=nation, unitTypeId=unit_type,
                formationId=("warsawPact." if row["nation"] in ["SO", "EG", "CZ", "PO"] else "nato.") + normalize(row["legend"] or row["designation"]),
                traits=traits, steps=steps)


def nearest_hex(x, y, hexes):
    # VASSAL CENTAG starts at y=3620; the traced canvas uses y=3622.
    if y >= 3620:
        y += 2
    target = min(hexes, key=lambda h: (h["x"] - x)**2 + (h["y"] - y)**2)
    assert (target["x"]-x)**2 + (target["y"]-y)**2 < 100, (x, y, target["id"])
    return target["id"]


# House rule replacing Reinforcement Boxes: each sector's units enter at the
# land map-edge hex nearest its printed box (see ReinforcementSector in map.rs).
SECTOR_HEXES = {1: "3534", 2: "4734", 3: "4301", 4: "3301", 5: "2501"}
# RR units have no printed sector; each formation uses the sector its G#
# units use, or the one nearest its home region: Czechoslovak/Carpathian
# fronts south (3), Belorussian front and the armies behind the GSFG centre
# (4), Polish and Baltic fronts north (5); British forces sector 1, French 2.
RAIL_SECTORS = {"CZF": 3, "CAF": 3, "BEF": 4, "5G": 4, "NEGF": 4, "SEGF": 4,
                "POF": 5, "BAF": 5, "BR I": 1, "FR I": 2, "FAR": 2}


def rail_sector(row):
    key = row["legend"] or row["designation"].split("/")[-1]
    if key in RAIL_SECTORS:
        return RAIL_SECTORS[key]
    return {"CZ": 3, "PO": 5, "FR": 2}.get(row["nation"], 4 if row["nation"] in ("SO", "EG") else 1)


def arrival_entry(row):
    """Arrival hex (None for the Strategic Reserve) and train marker for a reinforcement."""
    code = row["entry"].split()
    kind = code[0] if code else ""
    if re.fullmatch(r"G\d", kind):
        return dict(hex=SECTOR_HEXES[int(kind[1])])
    if kind in ("RS", "RF") and len(code) > 1:
        return dict(hex=code[1])
    if kind == "RR":
        return dict(hex=SECTOR_HEXES[rail_sector(row)], entrained=True)
    return dict(hex=None)  # Air (A), Sea (S), and EB units start in the Strategic Reserve.


def arrival(piece, family):
    x, y = piece["x"], piece["y"]
    if family == "extended-buildup":
        if y > 650:
            return None  # Optional XVIII Corps intervention requires a player decision.
        row = min(range(3), key=lambda i: abs(y - [283, 418, 550][i]))
        col = min(range(4), key=lambda i: abs(x - [128, 440, 756, 1066][i]))
        return row * 4 + col + 1
    if piece["map"] == "PACT Reinforcements":
        if y > 1340:
            # These are mobilization-track arrivals for War of Nerves, not
            # fixed war-turn arrivals. Preserve them separately for future rules.
            if x < 160:
                return 13 if y < 1450 else 14
            return min([(15, 305), (16, 564), (17, 839), (18, 1090)], key=lambda p: abs(x-p[1]))[0]
        return min([(1, 233), (2, 383), (3, 508), (4, 641), (6, 763),
                    (7, 897), (8, 1030), (9, 1160), (10, 1290)], key=lambda p: abs(y-p[1]))[0]
    if y > 1450:
        year = 1983 if "83" in family else 1988
        centers = [(18, 83), (19, 281), (21, 483), (22, 680), (23, 959), (24, 1155)] if year == 1983 else [(15, 130), (18, 472), (19, 667), (22, 868), (25, 1090)]
        return min(centers, key=lambda p: abs(x-p[1]))[0]
    return min([(2, 310), (3, 475), (4, 645), (5, 823), (6, 985),
                (7, 1118), (8, 1250), (9, 1378)], key=lambda p: abs(y-p[1]))[0]


def main():
    catalogs = read_catalog()
    root = ET.parse(MODULE / "buildFile.xml").getroot()
    slots = {s.get("gpid"): s for s in root.iter() if s.tag.endswith(".PieceSlot")}
    hexes = json.loads((ROOT / "crates/ooaw-core/data/natoMap.json").read_text())["hexes"]
    for h in hexes:
        h.update(x=15 + 133.5 * (38-h["col"]) + (66.75 if h["row"] % 2 else 0),
                 y=-3 + 114.4*h["row"])
    result = dict(source="NATO VASSAL 2.4.1; living Play Booklet 1 January 2026, sections 37, 38, 40, 44", units={}, scenarios={})
    for family, prefix in [("strategic-surprise", "SS"), ("extended-buildup", "EB"), ("war-of-nerves", "WoN")]:
        for year in [1983, 1988]:
            setup = prefix + str(year)[-2:]
            entries, deferred = [], []
            for piece in read_pieces(setup, slots):
                image = counter_image(piece, year)
                owner = piece["name"].split()[0]
                wp = owner in ["SO", "EG", "CZ", "PO"]
                catalog = catalogs["WP88" if year == 1988 else "WP"] if wp else catalogs[str(year)[-2:]]
                row = identify(piece, image, catalog)
                unit = unit_data(row)
                key = str(year) + ":" + unit["id"]
                result["units"][key] = unit
                if piece["map"] in ["NATO Reinforcements", "PACT Reinforcements"]:
                    turn = arrival(piece, family + str(year) if family != "extended-buildup" else family)
                    record = dict(unit=key, gameTurn=turn, **arrival_entry(row))
                    if turn is None or turn > 14:
                        deferred.append(record)
                        continue
                    entries.append(record)
                else:
                    # Pieces in the printed Strategic Reserve boxes stay in reserve.
                    # Reforger units join the ground reinforcements (house rule)
                    # and stand at their Reforger site.
                    reserve = piece["x"] > 5200 or piece["x"] < 420
                    location = None if reserve else nearest_hex(piece["x"], piece["y"], hexes)
                    if family != "extended-buildup" and row["entry"].startswith("RF"):
                        location = arrival_entry(row)["hex"]
                    if family != "extended-buildup" and owner == "WG" and normalize(row["designation"]) == "9/3pz":
                        location = "2716"  # Living erratum 37.3/40.3.
                    entries.append(dict(unit=key, gameTurn=1, hex=location))
            entries.sort(key=lambda e: (e["gameTurn"], e["unit"]))
            deferred.sort(key=lambda e: e["unit"])
            result["scenarios"][f"nato-{family}-{year}"] = dict(sourceSetup=setup+"_2.4_1.vsav", units=entries, deferredUnits=deferred)
            print(setup, len(entries), "scheduled units;", len(deferred), "deferred/optional")
    # One catalog unit/deployment per line keeps the large reference dataset
    # reviewable while retaining all fields and deterministic ordering.
    lines = ["{", '  "source": ' + json.dumps(result["source"]) + ",", '  "units": {']
    units = list(sorted(result["units"].items()))
    for i, (key, unit) in enumerate(units):
        lines.append("    " + json.dumps(key) + ": " + json.dumps(unit) +
                     ("," if i+1 < len(units) else ""))
    lines += ["  },", '  "scenarios": {']
    scenarios = list(result["scenarios"].items())
    for i, (key, setup) in enumerate(scenarios):
        lines += ["    " + json.dumps(key) + ": {",
                  '      "sourceSetup": ' + json.dumps(setup["sourceSetup"]) + ","]
        for field in ["units", "deferredUnits"]:
            lines.append("      " + json.dumps(field) + ": [")
            for j, record in enumerate(setup[field]):
                lines.append("        " + json.dumps(record) +
                             ("," if j+1 < len(setup[field]) else ""))
            lines.append("      ]" + ("," if field == "units" else ""))
        lines.append("    }" + ("," if i+1 < len(scenarios) else ""))
    lines += ["  }", "}"]
    OUTPUT.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
