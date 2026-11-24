#!/usr/bin/env bash
# Regenerate the app icon PNGs from the SVG in each icon folder (run after editing an SVG):
#   tools/render-icons.sh [dir...]   (default: every adx app icon folder)
# Each folder holds one NAME.svg (and optionally NAME-small.svg, a simplified
# drawing for 16-24 px); the PNGs (<size>.png) are installed into the hicolor icon
# theme by the owning module (lib/common.sh adx_icon_install). The console's PNGs are also
# its page favicon (Chromium uses it as the window icon: panel, Alt+Tab).
# Needs rsvg-convert (apt: librsvg2-bin).
set -euo pipefail
root="$(dirname "$(readlink -f "$0")")/.."
command -v rsvg-convert >/dev/null || { echo "rsvg-convert missing (sudo apt install librsvg2-bin)" >&2; exit 1; }
[ $# -gt 0 ] || set -- "$root/tools/console/static/app-icon" "$root/modules/emulator/icon" "$root/modules/devices/icon"
for dir in "$@"; do
  svgs=()
  for f in "$dir"/*.svg; do case "$f" in *-small.svg) ;; *) [ -f "$f" ] && svgs+=("$f") ;; esac; done
  [ "${#svgs[@]}" = 1 ] || { echo "$dir: expected exactly one NAME.svg (plus an optional NAME-small.svg)" >&2; exit 1; }
  svg="${svgs[0]}" small="${svgs[0]%.svg}-small.svg"
  for s in 16 22 24 32 48 64 128 256 512; do
    from="$svg"
    if [ "$s" -le 24 ] && [ -f "$small" ]; then from="$small"; fi
    rsvg-convert -w "$s" -h "$s" "$from" -o "$dir/$s.png"
  done
  echo "rendered $(basename "$svg") in $dir"
done
echo "then reinstall: adx console install / adx emulator desktop / adx devices install"
