/**
 * Animated command-centre backdrop shared by the menu screens: a hex grid,
 * the front line with opposing unit contacts, a radar sweep, and scanlines.
 * Purely decorative.
 */
export function MenuBackdrop() {
  return (
    <div className="menu-backdrop" aria-hidden="true">
      <div className="menu-grid" />
      <svg className="menu-front" viewBox="0 0 600 900" preserveAspectRatio="xMidYMid slice">
        <defs>
          <filter id="menu-glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="6" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <path
          className="menu-front-line"
          d="M330 -20 L300 90 L338 170 L292 255 L318 340 L270 430 L310 520 L262 610 L300 700 L250 790 L280 920"
          filter="url(#menu-glow)"
        />
        {[
          [220, 140], [190, 320], [215, 470], [170, 640], [205, 760],
        ].map(([x, y], index) => (
          <rect key={`nato-${index}`} className="menu-contact nato" x={x} y={y} width="26" height="18" rx="2" style={{ animationDelay: `${index * 0.7}s` }} />
        ))}
        {[
          [390, 110], [420, 280], [380, 400], [410, 560], [360, 700], [430, 820],
        ].map(([x, y], index) => (
          <rect key={`pact-${index}`} className="menu-contact pact" x={x} y={y} width="26" height="18" rx="2" style={{ animationDelay: `${index * 0.55 + 0.3}s` }} />
        ))}
        <g className="menu-thrust">
          <path d="M405 300 C350 320 330 340 300 360" />
          <path d="M292 355 L306 346 L304 364 Z" />
        </g>
      </svg>
      <div className="menu-radar">
        <div className="menu-radar-sweep" />
        <span className="menu-blip" style={{ top: "34%", left: "62%" }} />
        <span className="menu-blip" style={{ top: "58%", left: "30%", animationDelay: "1.4s" }} />
        <span className="menu-blip" style={{ top: "70%", left: "66%", animationDelay: "2.6s" }} />
      </div>
      <div className="menu-scanlines" />
      <div className="menu-vignette" />
    </div>
  );
}
