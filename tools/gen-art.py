#!/usr/bin/env python3
"""Generate the app's device and battery drawings.

Every drawing here is original geometry in a flat style. Animations are frame sequences:
librsvg (the SVG renderer GTK uses) does not play SMIL animation, so the UI cycles frames.

Run from the repository root:
    python3 tools/gen-art.py

Writes into spacepods-ui/resources/devices/ and spacepods-ui/resources/battery/.
"""

import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "spacepods-ui" / "resources"
DEVICES = ROOT / "devices"
ANIM = DEVICES / "anim"
BATTERY = ROOT / "battery"

FILL = "#e8ebf0"
STROKE = "#7d8590"
HIGHLIGHT = "#b5bcc6"
LED = "#3ec46d"


def svg(body: str, view_box: str = "0 0 120 120") -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}">\n'
        f"{body}\n"
        "</svg>\n"
    )


def earbud(x: float, y: float, scale: float = 1.0) -> str:
    """One earbud, centred on (x, y): a rounded pod with a short stem."""
    return (
        f'<g transform="translate({x} {y}) scale({scale})">'
        f'<rect x="-15" y="-30" width="30" height="40" rx="14" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
        f'<rect x="-7" y="6" width="14" height="28" rx="7" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
        f'<path d="M-8 -20 H8" stroke="{HIGHLIGHT}" stroke-width="2" stroke-linecap="round"/>'
        "</g>"
    )


def case_base() -> str:
    """The lower part of a charging case, drawn in front of the earbuds."""
    return (
        f'<rect x="20" y="58" width="80" height="44" rx="14" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
        f'<circle cx="60" cy="92" r="2.5" fill="{LED}"/>'
    )


def case_lid_open() -> str:
    """The lid, raised behind the earbuds."""
    return f'<rect x="20" y="22" width="80" height="34" rx="14" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'


def earbuds_pair() -> str:
    return earbud(42, 60, 0.95) + earbud(78, 60, 0.95)


def headphones_body(origin_x: float = 60) -> str:
    cx = origin_x
    return (
        f'<path d="M{cx - 30} 70 V56 A30 30 0 0 1 {cx + 30} 56 V70" fill="none" stroke="{STROKE}" stroke-width="5" stroke-linecap="round"/>'
        f'<rect x="{cx - 38}" y="60" width="18" height="30" rx="7" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
        f'<rect x="{cx + 20}" y="60" width="18" height="30" rx="7" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
    )


def neckband_body() -> str:
    return (
        f'<path d="M30 42 Q30 92 60 92 Q90 92 90 42" fill="none" stroke="{STROKE}" stroke-width="5" stroke-linecap="round"/>'
        f'<rect x="22" y="30" width="16" height="22" rx="7" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
        f'<rect x="82" y="30" width="16" height="22" rx="7" fill="{FILL}" stroke="{STROKE}" stroke-width="2"/>'
    )


def ease_in(t: float) -> float:
    """Slow start, fast finish."""
    return t * t * t


def spin_frame(body: str, theta: float) -> str:
    """Flat spin: squeeze horizontally around the centre, as a turning object appears."""
    scale = max(0.06, abs(math.cos(theta)))
    return (
        f'<g transform="translate(60 60) scale({scale:.3f} 1) translate(-60 -60)">'
        f"{body}</g>"
    )


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def main() -> None:
    write(DEVICES / "earbuds.svg", svg(earbuds_pair()))
    write(DEVICES / "earbuds-case.svg", svg(case_lid_open() + earbuds_pair() + case_base()))
    write(DEVICES / "headphones.svg", svg(headphones_body(), "0 0 120 120"))
    write(DEVICES / "neckband.svg", svg(neckband_body()))

    frames = 12
    for i in range(frames):
        t = i / (frames - 1)
        rise = 34 * ease_in(t)  # pods move up out of the case, starting slowly
        # Clip the pods to the case's outline: stems stay hidden while the pods are inside.
        clip = '<clipPath id="case"><rect x="0" y="0" width="120" height="102"/></clipPath>'
        pods = (
            f'<g clip-path="url(#case)">'
            + earbud(42, 84 - rise, 0.95)
            + earbud(78, 84 - rise, 0.95)
            + "</g>"
        )
        write(ANIM / f"earbuds-out-{i:02d}.svg", svg(clip + case_lid_open() + pods + case_base()))

    frames = 24
    for i in range(frames):
        theta = 2 * math.pi * i / frames
        write(ANIM / f"headphones-spin-{i:02d}.svg", svg(spin_frame(headphones_body(), theta)))
        write(ANIM / f"neckband-spin-{i:02d}.svg", svg(spin_frame(neckband_body(), theta)))

    levels = {
        "full": (58, "#2ec27e"),
        "three-quarters": (43.5, "#2ec27e"),
        "half": (29, "#f6d32d"),
        "quarter": (14.5, "#ff7800"),
        "empty": (0, "#e01b24"),
        # Battery unknown: neutral grey outline, no level. Never guessed.
        "unknown": (0, "#9aa1ac"),
    }
    for name, (width, color) in levels.items():
        body_color = "#5d636d" if name != "empty" else "#e01b24"
        inner = (
            f'<rect x="7" y="11" width="{width}" height="18" rx="4" fill="{color}"/>'
            if width > 0
            else ""
        )
        text = (
            f'<rect x="2" y="6" width="68" height="28" rx="8" fill="none" stroke="{body_color}" stroke-width="3"/>'
            f'<rect x="72" y="15" width="5" height="10" rx="2.5" fill="{body_color}"/>'
            f"{inner}"
        )
        write(BATTERY / f"battery-{name}.svg", svg(text, "0 0 80 40"))


def write_resources() -> None:
    """Rewrite the resource bundle so every drawing and frame is registered.

    The icon entries from before are kept as they were; the device and battery entries are
    listed from the files that exist, in a stable order.
    """
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        "<gresources>",
        '    <gresource prefix="/com/spacepods/ui/icons/scalable/actions">',
        '        <file alias="hearing-health-symbolic.svg">icons/hicolor/scalable/actions/hearing-health-symbolic.svg</file>',
        '        <file alias="settings-symbolic.svg">icons/hicolor/scalable/actions/settings-symbolic.svg</file>',
        '        <file alias="gesture-swipe-right-symbolic.svg">icons/hicolor/scalable/actions/gesture-swipe-right-symbolic.svg</file>',
        '        <file alias="com.github.imnotndesh.spacepods-linux.svg">icons/hicolor/scalable/actions/com.github.imnotndesh.spacepods-linux.svg</file>',
        "    </gresource>",
        '    <gresource prefix="/com/github/imnotndesh/spacepods-linux/devices">',
    ]
    for f in sorted(DEVICES.glob("*.svg")):
        lines.append(f'        <file alias="{f.name}">devices/{f.name}</file>')
    for f in sorted(ANIM.glob("*.svg")):
        lines.append(f'        <file alias="anim/{f.name}">devices/anim/{f.name}</file>')
    lines.append("    </gresource>")
    lines.append('    <gresource prefix="/com/github/imnotndesh/spacepods-linux/battery">')
    for f in sorted(BATTERY.glob("*.svg")):
        lines.append(f'        <file alias="{f.name}">battery/{f.name}</file>')
    lines.append("    </gresource>")
    lines.append("</gresources>")
    (ROOT / "resources.gresource.xml").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
    write_resources()
