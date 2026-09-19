"""Render app icon as the in-app .mark: rounded gray tile + fish, transparent outside."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SVG_PATH = ROOT / "src-tauri" / "app-icon.svg"

# Match .mark: --active over --rail ≈ #262626; glyph --ink
MARK_BG = "#262626"
INK = "#eceae6"
FISH = (
    "M4 14c2.2-4 4.4-6 8-6 2.4 0 4.2 1 6.2 3.1L20 9.2"
    "C17.6 6.4 14.8 5 12 5 7.2 5 4.2 8.2 2 14c1.6 2.8 3.6 4.6 6.4 5.4"
    " 1.2.3 2.4.4 3.6.2 2.6-.4 4.6-1.8 6.8-4.2l-1.6-1.2"
    "c-1.8 2-3.4 3-5.4 3.3-2.4.4-4.4-.6-6.2-3.5Z"
)

# Transparent canvas + rounded tile (8/28 ≈ 28.5% radius). No opaque black frame.
SVG = f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <rect x="48" y="48" width="928" height="928" rx="266" ry="266" fill="{MARK_BG}"/>
  <g transform="translate(208 208) scale(25.3333)">
    <path fill="{INK}" d="{FISH}"/>
  </g>
</svg>
"""


def main() -> None:
    SVG_PATH.write_text(SVG, encoding="utf-8")
    print(f"wrote {SVG_PATH}")


if __name__ == "__main__":
    main()
