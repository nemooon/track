#!/usr/bin/env python3
"""Draws the app icon (packaging/icon.svg): a stopwatch face (no buttons) on a dark tile under the
header's dot pattern, its tracked time marked in the app's key colour (a blue-leaning green). scripts/build-icon.sh renders it into icon.icns.

Shadows are stacked translucent shapes, not SVG filters (macOS's SVG renderer ignores filters).
"""
import math
import random
import sys

CX, CY = 512, 512  # the watch's centre: the middle of the icon
R = 292  # the watch's outer radius (before WATCH_SCALE)
# The whole watch (with its shadow) is drawn this much smaller, about the centre.
WATCH_SCALE = 0.9
# The tracked time and the hand's cap: the app's key colour (Palette::dark().accent).
ACCENT = "#4cc9b8"
TILE = (100, 100, 824, 185)  # x, y, side, corner radius (Apple's icon grid)


def polar(r, deg):
    a = math.radians(deg - 90)
    return CX + r * math.cos(a), CY + r * math.sin(a)


def arc(r, a0, a1):
    (x0, y0), (x1, y1) = polar(r, a0), polar(r, a1)
    large = 1 if (a1 - a0) % 360 > 180 else 0
    return f"M{x0:.1f} {y0:.1f} A{r} {r} 0 {large} 1 {x1:.1f} {y1:.1f}"


def soft_shadow(shape, steps=8, dy=4, alpha=0.05):
    """`shape(i)` returns an SVG element string for layer i (grown and lowered a little each time)."""
    return "".join(shape(i, i * dy, alpha) for i in range(steps, 0, -1))


def ticks():
    out = []
    for i in range(60):
        major = i % 5 == 0
        (x1, y1), (x2, y2) = polar(240, i * 6), polar(204 if major else 226, i * 6)
        colour, width = ("#eef0f3", 13) if major else ("#6b7280", 5)
        out.append(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{colour}" stroke-width="{width}" stroke-linecap="round"/>')
    return "".join(out)


def dots():
    """The header's pattern: square dots of random depth, fading out toward the left."""
    random.seed(7)
    out = []
    x0, y0, side, _ = TILE
    for gx in range(x0 + 6, x0 + side, 24):
        for gy in range(y0 + 6, y0 + side, 24):
            fade = 0.25 + 0.75 * (gx - x0) / side
            o = random.random() ** 2.2 * 0.26 * fade
            if o > 0.01:
                out.append(f'<rect x="{gx}" y="{gy}" width="9" height="9" fill="#fff" fill-opacity="{o:.3f}"/>')
    return "".join(out)


def svg():
    x0, y0, side, rx = TILE
    hand_end, hand_tail = polar(206, 130), polar(-44, 130)
    tile_shadow = soft_shadow(lambda i, dy, a: f'<rect x="{x0 - i}" y="{y0 + dy}" width="{side + 2 * i}" height="{side}" rx="{rx + i}" fill="#000" fill-opacity="{a}"/>', steps=6, dy=3, alpha=0.035)
    watch_shadow = soft_shadow(lambda i, dy, a: f'<circle cx="{CX}" cy="{CY + 6 + dy}" r="{R + i * 2}" fill="#000" fill-opacity="{a}"/>', steps=7, dy=3, alpha=0.06)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
<defs>
<linearGradient id="tile" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#343842"/><stop offset="1" stop-color="#101216"/></linearGradient>
<clipPath id="tileclip"><rect x="{x0}" y="{y0}" width="{side}" height="{side}" rx="{rx}"/></clipPath>
<linearGradient id="metal" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fbfcfd"/><stop offset="0.55" stop-color="#d9dde4"/><stop offset="1" stop-color="#aeb4bf"/></linearGradient>
<radialGradient id="face" cx="0.5" cy="0.35" r="0.75"><stop offset="0" stop-color="#232730"/><stop offset="1" stop-color="#0c0e11"/></radialGradient>
</defs>
{tile_shadow}
<rect x="{x0}" y="{y0}" width="{side}" height="{side}" rx="{rx}" fill="url(#tile)"/>
<g clip-path="url(#tileclip)" transform="rotate(-2 512 512)">{dots()}</g>
<rect x="{x0 + 1.5}" y="{y0 + 1.5}" width="{side - 3}" height="{side - 3}" rx="{rx - 1.5}" fill="none" stroke="#fff" stroke-opacity="0.10" stroke-width="3"/>
<g transform="translate({CX} {CY}) scale({WATCH_SCALE}) translate({-CX} {-CY})">
{watch_shadow}
<circle cx="{CX}" cy="{CY}" r="{R}" fill="url(#metal)"/>
<circle cx="{CX}" cy="{CY}" r="262" fill="#6b7280"/>
<circle cx="{CX}" cy="{CY}" r="256" fill="url(#face)"/>
<path d="{arc(214, 0, 130)}" fill="none" stroke="{ACCENT}" stroke-opacity="0.25" stroke-width="66" stroke-linecap="round"/>
<path d="{arc(214, 0, 130)}" fill="none" stroke="{ACCENT}" stroke-width="40" stroke-linecap="round"/>
{ticks()}
<line x1="{hand_tail[0]:.1f}" y1="{hand_tail[1]:.1f}" x2="{hand_end[0]:.1f}" y2="{hand_end[1]:.1f}" stroke="#ffffff" stroke-width="15" stroke-linecap="round"/>
<circle cx="{CX}" cy="{CY}" r="26" fill="#ffffff"/>
<circle cx="{CX}" cy="{CY}" r="11" fill="{ACCENT}"/>
</g>
</svg>
'''


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "packaging/icon.svg"
    with open(out, "w") as f:
        f.write(svg())
