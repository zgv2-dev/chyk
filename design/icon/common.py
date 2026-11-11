"""Shared look of the Chyk icon concepts: palette, SVG defs, and the macOS tile."""

G = "#4dff9b"  # phosphor green
G2 = "#1f8f55"
INK = "#00140a"

# macOS icon grid: 824 px rounded square centered on a 1024 canvas, ~185 px corner radius.
DEFS = f"""
<defs>
  <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#0b1d12"/><stop offset="1" stop-color="#020604"/>
  </linearGradient>
  <radialGradient id="core" cx="0.5" cy="0.5" r="0.5">
    <stop offset="0" stop-color="{G}" stop-opacity="0.22"/><stop offset="1" stop-color="{G}" stop-opacity="0"/>
  </radialGradient>
  <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
    <feGaussianBlur stdDeviation="14" result="b"/>
    <feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
  </filter>
  <filter id="soft" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="6"/></filter>
  <pattern id="dots" width="36" height="36" patternUnits="userSpaceOnUse">
    <circle cx="18" cy="18" r="2" fill="{G}" fill-opacity="0.07"/>
  </pattern>
  <clipPath id="tile"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath>
  <filter id="shadow" filterUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024">
    <feGaussianBlur stdDeviation="22"/>
  </filter>
</defs>"""

TILE = f"""
<rect x="100" y="118" width="824" height="824" rx="185" fill="#000" fill-opacity="0.45" filter="url(#shadow)"/>
<rect x="100" y="100" width="824" height="824" rx="185" fill="url(#bg)"/>
<g clip-path="url(#tile)">
  <rect x="100" y="100" width="824" height="824" fill="url(#dots)"/>
  <circle cx="512" cy="512" r="330" fill="url(#core)"/>
</g>
<rect x="101.5" y="101.5" width="821" height="821" rx="184" fill="none" stroke="{G}" stroke-opacity="0.16" stroke-width="3"/>"""
