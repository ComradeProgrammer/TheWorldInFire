import type { ScenarioSummary } from "../gameApi";

/**
 * Presentation-only grouping for the scenario screen: which family a
 * registered scenario belongs to, its year variant, and a short blurb. The
 * core stays the authority on which scenarios exist; a registered scenario
 * missing from this catalog still appears, under "Other".
 */
interface FamilyEntry {
  key: string;
  group: string;
  title: string;
  blurb: string;
  /** Scenario IDs by year, in display order. */
  variants: { year: string; id: string }[];
}

const CATALOG: FamilyEntry[] = [
  {
    key: "baltap",
    group: "Introductory",
    title: "BALTAP",
    blurb: "The northern flank: Warsaw Pact forces strike into Schleswig-Holstein and Denmark. Seven turns and a few dozen units, the place to learn the game.",
    variants: [{ year: "1983", id: "nato-baltap-1983" }],
  },
  {
    key: "strategic-surprise",
    group: "Campaigns",
    title: "Strategic Surprise",
    blurb: "The Warsaw Pact attacks across the whole Central Front with little warning. Its attacks gain the Surprise modifier on the first war turn.",
    variants: [
      { year: "1983", id: "nato-strategic-surprise-1983" },
      { year: "1988", id: "nato-strategic-surprise-1988" },
    ],
  },
  {
    key: "extended-buildup",
    group: "Campaigns",
    title: "Extended Buildup",
    blurb: "Both alliances mobilize before the shooting starts: more forces are on the map at the outset, more airlift is available, and more HQs can give Offensive Support.",
    variants: [
      { year: "1983", id: "nato-extended-buildup-1983" },
      { year: "1988", id: "nato-extended-buildup-1988" },
    ],
  },
  {
    key: "war-of-nerves",
    group: "Campaigns",
    title: "War of Nerves",
    blurb: "A crisis escalates into war along the Central Front. Play begins on the first war turn; the peacetime escalation steps are not part of this version yet.",
    variants: [
      { year: "1983", id: "nato-war-of-nerves-1983" },
      { year: "1988", id: "nato-war-of-nerves-1988" },
    ],
  },
  {
    key: "rules-prototype",
    group: "Development",
    title: "Rules Prototype",
    blurb: "A two-unit test bed used while developing the rules.",
    variants: [{ year: "1983", id: "nato-1983-standard" }],
  },
];

const GROUP_ORDER = ["Introductory", "Campaigns", "Development", "Other"];

/** A scenario family with the variants the core actually registers. */
export interface ScenarioFamily {
  key: string;
  group: string;
  title: string;
  blurb: string | null;
  variants: { year: string | null; scenario: ScenarioSummary }[];
}

/** Groups registered scenarios into families, in display order. */
export function scenarioFamilies(scenarios: ScenarioSummary[]): ScenarioFamily[] {
  const byId = new Map(scenarios.map((scenario) => [scenario.id, scenario]));
  const listed = new Set<string>();
  const families: ScenarioFamily[] = [];
  for (const entry of CATALOG) {
    const variants = entry.variants.flatMap(({ year, id }) => {
      const scenario = byId.get(id);
      if (!scenario) return [];
      listed.add(id);
      return [{ year, scenario }];
    });
    if (variants.length > 0) families.push({ key: entry.key, group: entry.group, title: entry.title, blurb: entry.blurb, variants });
  }
  for (const scenario of scenarios) {
    if (listed.has(scenario.id)) continue;
    families.push({ key: scenario.id, group: "Other", title: scenario.name, blurb: null, variants: [{ year: null, scenario }] });
  }
  return families.sort((a, b) => GROUP_ORDER.indexOf(a.group) - GROUP_ORDER.indexOf(b.group));
}
