import type { UnitState } from "../gameApi";
import {
  COUNTER,
  counterPalette,
  FRAME,
  NATION_COLORS,
  nationCode,
  nationInk,
  outOfSupply,
  RESERVE_TAB,
  reserveTabLabel,
  SUPPLY_TAB,
  svgPathData,
  unitSymbol,
} from "../map/unitSymbols";

function css(color: number): string {
  return `#${color.toString(16).padStart(6, "0")}`;
}

/**
 * The unit's APP-6 symbol (frame, icon, modifier, HQ staff) in symbol space.
 * `strokeScale` converts the desired stroke width to symbol units.
 */
export function UnitSymbolPaths({ unit, ink, strokeScale, echelon = true }: {
  unit: UnitState;
  ink: string;
  strokeScale: number;
  echelon?: boolean;
}) {
  const symbol = unitSymbol(unit);
  return (
    <g fill="none" stroke={ink} strokeLinecap="round" strokeLinejoin="round">
      <rect x={FRAME.x1} y={FRAME.y1} width={FRAME.x2 - FRAME.x1} height={FRAME.y2 - FRAME.y1} strokeWidth={2.4 * strokeScale} />
      {symbol.strokes.length > 0 && <path d={svgPathData(symbol.strokes)} strokeWidth={2 * strokeScale} />}
      {echelon && symbol.echelon.length > 0 && <path d={svgPathData(symbol.echelon)} strokeWidth={1.8 * strokeScale} />}
      {symbol.headquarters && (
        <line
          x1={FRAME.x1}
          y1={FRAME.y2}
          x2={FRAME.x1}
          y2={FRAME.y2 + COUNTER.staffLength / COUNTER.symbolScale}
          strokeWidth={2.4 * strokeScale}
        />
      )}
    </g>
  );
}

/**
 * The same counter the map draws, as an SVG for the React UI. `x` and `y`
 * place it when it is nested inside another SVG; `reserve` adds the
 * Reserve/OMG Marker tab.
 */
export function UnitCounterIcon({ unit, size = 64, x, y, reserve = false }: {
  unit: UnitState;
  size?: number;
  x?: number;
  y?: number;
  reserve?: boolean;
}) {
  const palette = counterPalette(unit.sideId);
  const nationColor = NATION_COLORS[unit.nationId] ?? palette.edge;
  const step = unit.steps[unit.strengthStepIndex];
  const half = COUNTER.size / 2;
  const { band } = COUNTER;
  const scale = COUNTER.symbolScale;
  const symbol = unitSymbol(unit);
  const { reserveTab: tab, supplyTab } = COUNTER;
  const unsupplied = outOfSupply(unit);
  // Room around the counter for marker tabs when they are shown.
  const below = reserve ? tab.y + tab.height - half : 0;
  const above = unsupplied ? -supplyTab.y - half - 3 : 0;
  const extra = below + above;
  const box = COUNTER.size + 6;
  return (
    <svg
      className="unit-counter-icon"
      x={x}
      y={y}
      width={size}
      height={(size * (box + extra)) / box}
      viewBox={`${-half - 3} ${-half - 3 - above} ${box} ${box + extra}`}
      role="img"
      aria-label={`${unit.name} counter`}
    >
      <rect x={-half} y={-half} width={COUNTER.size} height={COUNTER.size} rx={5} fill={css(palette.fill)} stroke="#05080c" strokeOpacity={0.72} strokeWidth={5} />
      <rect x={-half} y={-half} width={COUNTER.size} height={COUNTER.size} rx={5} fill="none" stroke={css(palette.edge)} strokeWidth={2} />
      <rect x={band.x} y={band.y} width={band.width} height={band.height} fill={css(nationColor)} />
      <text x={0} y={band.y + band.height / 2} className="counter-icon-text" fontSize={10} fill={css(nationInk(nationColor))}>
        {nationCode(unit.nationId)}
      </text>
      <g transform={`translate(0 ${COUNTER.symbolY}) scale(${scale}) translate(-100 -100)`}>
        <UnitSymbolPaths unit={unit} ink={css(palette.ink)} strokeScale={1 / scale} />
      </g>
      {symbol.label && (
        <text x={0} y={COUNTER.symbolY} className="counter-icon-text" fontSize={14} fill={css(palette.ink)}>{symbol.label}</text>
      )}
      {step && (
        <text x={0} y={COUNTER.valuesY} className="counter-icon-text" fontSize={18} fill={css(palette.ink)}>
          {`${step.attack}  ${step.defense}  ${step.movement}`}
        </text>
      )}
      {unit.disruption && (
        <>
          <circle cx={-half + 6} cy={half - 6} r={15} fill="#ff9f1c" stroke="#05080c" strokeWidth={3} />
          <text x={-half + 6} y={half - 6} className="counter-icon-text" fontSize={17} fill="#05080c">
            {unit.disruption === "suppressed" ? "S" : "D"}
          </text>
        </>
      )}
      {unsupplied && (
        <>
          <rect x={-supplyTab.width / 2} y={supplyTab.y} width={supplyTab.width} height={supplyTab.height} rx={3} fill={css(SUPPLY_TAB.fill)} stroke="#05080c" strokeWidth={2} />
          <text x={0} y={supplyTab.y + supplyTab.height / 2} className="counter-icon-text" fontSize={10} fill={css(SUPPLY_TAB.ink)}>
            OOS
          </text>
        </>
      )}
      {reserve && (
        <>
          <rect x={-tab.width / 2} y={tab.y} width={tab.width} height={tab.height} rx={3} fill={css(RESERVE_TAB.fill)} stroke="#05080c" strokeWidth={2} />
          <text x={0} y={tab.y + tab.height / 2} className="counter-icon-text" fontSize={11} fill={css(RESERVE_TAB.ink)}>
            {reserveTabLabel(unit.sideId)}
          </text>
        </>
      )}
    </svg>
  );
}
