import type { UnitState } from "../gameApi";
import {
  COUNTER,
  counterPalette,
  FRAME,
  NATION_COLORS,
  nationCode,
  nationInk,
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

/** The same counter the map draws, as an SVG for the React UI. */
export function UnitCounterIcon({ unit, size = 64 }: { unit: UnitState; size?: number }) {
  const palette = counterPalette(unit.sideId);
  const nationColor = NATION_COLORS[unit.nationId] ?? palette.edge;
  const step = unit.steps[unit.strengthStepIndex];
  const half = COUNTER.size / 2;
  const { band } = COUNTER;
  const scale = COUNTER.symbolScale;
  return (
    <svg
      className="unit-counter-icon"
      width={size}
      height={size}
      viewBox={`${-half - 3} ${-half - 3} ${COUNTER.size + 6} ${COUNTER.size + 6}`}
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
    </svg>
  );
}
