"""The Chyk icon ("chyk" is the sound of scissors): the trim box cut in two, its halves tilted
apart, and white scissors in the cut where the playhead used to be."""
import math

from common import G, INK, TILE

TILT = 5  # degrees each half turns away from the cut
CUT, GAP = 470, 10  # cut position (the old playhead) and the gap at the top of the cut
SIZE = 0.75  # scissors scale, about the pivot
TOP = 334  # top of the box; the box sits a little high to leave room for the scissor rings


def half(left):
    """One half of the trim box: the rounded handle with grips outside, the straight cut inside."""
    x = CUT - GAP / 2 if left else CUT + GAP / 2
    if left:
        d = (f"M{x},{TOP} H308 a54,54 0 0 0 -54,54 v172 a54,54 0 0 0 54,54 H{x} Z "
             f"M{x},{TOP + 30} H318 v220 H{x} Z")
        lo, hi, grips = 318, x, (277, 295)
    else:
        d = (f"M{x},{TOP} H716 a54,54 0 0 1 54,54 v172 a54,54 0 0 1 -54,54 H{x} Z "
             f"M{x},{TOP + 30} H706 v220 H{x} Z")
        lo, hi, grips = x, 706, (729, 747)
    # The box's four frames, clipped to this half, on a dark backing (the color that showed
    # through before) so the blade behind the film barely shows.
    frames = f'<rect x="{lo}" y="{TOP + 30}" width="{hi - lo}" height="220" fill="#0d2a1b" fill-opacity="0.9"/>'
    for i in range(4):
        a, b = max(318 + i * 98, lo), min(406 + i * 98, hi)
        if b > a:
            frames += (f'<rect x="{a}" y="{TOP + 30}" width="{b - a}" height="220" rx="10" fill="{G}" '
                       f'fill-opacity="{0.10 + 0.05 * (i % 2)}"/>')
    lines = "".join(f'<line x1="{g}" y1="{TOP + 104}" x2="{g}" y2="{TOP + 176}"/>' for g in grips)
    # Turned about the cut's top corner: the cut is widest near the scissors' pivot.
    turn = TILT if left else -TILT
    return (f'<g transform="rotate({turn} {x} {TOP})">{frames}'
            f'<path d="{d}" fill="{G}" fill-rule="evenodd" filter="url(#glow)"/>'
            f'<g stroke="{INK}" stroke-width="9" stroke-linecap="round">{lines}</g></g>')


def scissors(color="#ffffff"):
    """Pointing up: the pivot just above the box's bottom edge, blades open along the cut, rings
    below the box. Returns (left blade, the rest): the left blade goes behind the film."""
    px, py = CUT, TOP + 240
    back, parts = [], []
    for side in (-1, 1):
        a = math.radians(TILT + 6) * side
        tip = (px + 300 * math.sin(a), py - 300 * math.cos(a))
        nx, ny = 24 * math.cos(a), 24 * math.sin(a)  # half the blade's width at the pivot
        (back if side < 0 else parts).append(f'<polygon points="{px - nx},{py - ny} {px + nx},{py + ny} {tip[0]},{tip[1]}" '
                     f'fill="{color}" stroke="{color}" stroke-width="8" stroke-linejoin="round"/>')
        # Each blade's shank crosses the pivot to the ring on the other side.
        rx, ry = px - side * 58, py + 156
        parts.append(f'<line x1="{px}" y1="{py}" x2="{rx}" y2="{ry - 40}" stroke="{color}" '
                     f'stroke-width="30" stroke-linecap="round"/>')
        parts.append(f'<circle cx="{rx}" cy="{ry}" r="44" fill="none" stroke="{color}" stroke-width="26"/>')
    parts.append(f'<circle cx="{px}" cy="{py}" r="11" fill="{INK}"/>')
    # Scaled inside the glow, so the glow keeps its width.
    at = f"translate({px} {py}) scale({SIZE}) translate({-px} {-py})"
    return tuple(f'<g filter="url(#glow)"><g transform="{at}">{"".join(x)}</g></g>' for x in (back, parts))


def snip():
    back, front = scissors()
    return TILE + back + half(True) + half(False) + front


def mark():
    """The icon without the tile: the app header's logo, on a transparent background."""
    back, front = scissors()
    return back + half(True) + half(False) + front
