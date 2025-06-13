export const PropertyGraphTopology = () => (
  <svg viewBox="0 0 320 220" className="mx-auto h-auto w-full max-w-lg" style={{ minHeight: '200px' }}>
    {/* Property nodes (top row) */}
    {/* Manufacturer property */}
    <circle cx="60" cy="40" r="12" fill="#F47500" stroke="#131313" strokeWidth="2" />
    <text x="60" y="13" textAnchor="middle" className="fill-content" fontSize="12" fontFamily="monospace">
      brand
    </text>
    <text x="60" y="23" textAnchor="middle" className="fill-content3" fontSize="10" fontFamily="monospace">
      string
    </text>

    {/* Year property */}
    <circle cx="160" cy="40" r="12" fill="#F47500" stroke="#131313" strokeWidth="2" />
    <text x="160" y="13" textAnchor="middle" className="fill-content" fontSize="12" fontFamily="monospace">
      year
    </text>
    <text x="160" y="23" textAnchor="middle" className="fill-content3" fontSize="10" fontFamily="monospace">
      number
    </text>

    {/* Color property */}
    <circle cx="260" cy="40" r="12" fill="#F47500" stroke="#131313" strokeWidth="2" />
    <text x="260" y="13" textAnchor="middle" className="fill-content" fontSize="12" fontFamily="monospace">
      color
    </text>
    <text x="260" y="23" textAnchor="middle" className="fill-content3" fontSize="10" fontFamily="monospace">
      string
    </text>

    {/* Connection lines from properties to records */}
    {/* Manufacturer to CAR */}
    <line x1="60" y1="52" x2="100" y2="128" stroke="#808080" strokeWidth="1" opacity="0.4" />
    {/* Year to CAR */}
    <line x1="160" y1="52" x2="100" y2="128" stroke="#808080" strokeWidth="1" opacity="0.4" />
    {/* Year to HOUSE */}
    <line x1="160" y1="52" x2="220" y2="128" stroke="#808080" strokeWidth="1" opacity="0.4" />
    {/* Color to CAR */}
    <line x1="260" y1="52" x2="100" y2="128" stroke="#808080" strokeWidth="1" opacity="0.4" />
    {/* Color to HOUSE */}
    <line x1="260" y1="52" x2="220" y2="128" stroke="#808080" strokeWidth="1" opacity="0.4" />

    {/* Record nodes (bottom row) */}
    {/* CAR record */}
    <circle cx="100" cy="140" r="16" fill="#F4B000" stroke="#131313" strokeWidth="2" />
    <text x="100" y="143.5" textAnchor="middle" className="fill-background" fontSize="12" fontWeight="bold">
      C
    </text>
    <text
      x="100"
      y="168"
      textAnchor="middle"
      className="fill-content"
      fontSize="10"
      fontFamily="monospace"
      fontWeight="bold"
    >
      CAR
    </text>

    {/* CAR properties */}
    <text x="65" y="180" textAnchor="start" className="fill-content3" fontSize="10" fontFamily="monospace">
      color: "red"
    </text>
    <text x="65" y="192" textAnchor="start" className="fill-content3" fontSize="10" fontFamily="monospace">
      year: 2023
    </text>
    <text x="65" y="204" textAnchor="start" className="fill-content3" fontSize="10" fontFamily="monospace">
      brand: "Ford"
    </text>

    {/* HOUSE record */}
    <circle cx="220" cy="140" r="16" fill="#BD93F9" stroke="#131313" strokeWidth="2" />
    <text x="220" y="143.5" textAnchor="middle" className="fill-background" fontSize="12" fontWeight="bold">
      H
    </text>
    <text
      x="220"
      y="168"
      textAnchor="middle"
      className="fill-content"
      fontSize="10"
      fontFamily="monospace"
      fontWeight="bold"
    >
      HOUSE
    </text>

    {/* HOUSE properties */}
    <text x="185" y="180" textAnchor="start" className="fill-content3" fontSize="10" fontFamily="monospace">
      color: "white"
    </text>
    <text x="185" y="192" textAnchor="start" className="fill-content3" fontSize="10" fontFamily="monospace">
      year: 1995
    </text>
  </svg>
)
