#!/usr/bin/env bash
# Regenerate the adx console icon PNGs from adx-console.svg (run after editing the SVG).
# The PNGs are the page favicon (Chromium uses it as the window icon: panel, Alt+Tab)
# and are installed into the hicolor icon theme by `adx console install` (menu entry).
# Needs rsvg-convert (apt: librsvg2-bin).
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
command -v rsvg-convert >/dev/null || { echo "rsvg-convert missing (sudo apt install librsvg2-bin)" >&2; exit 1; }
for s in 16 22 24 32 48 64 128 256 512; do
  rsvg-convert -w "$s" -h "$s" adx-console.svg -o "$s.png"
done
echo "icons regenerated; then: adx console install && adx console stop && adx console start"
