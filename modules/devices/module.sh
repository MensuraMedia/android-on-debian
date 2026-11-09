MOD_DESC="Phone image manager aodm + its window aod-gui (Rust): build with the pinned toolchain, install to ~/.local/bin, menu entry"

_ws()     { echo "$ADX_ROOT/tools/aodm"; }
_BINS="aodm aod-gui"
_bin()    { echo "$HOME/.local/bin/${1:-aodm}"; }
# Marker proving the installed binary is adx's: its SHA-256, written at install.
_marker() { echo "$ADX_LOG_DIR/../${1:-aodm}.installed"; }
_desktop_file() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/applications/aod-gui.desktop"; }
_ver()    { sed -nE '/^\[workspace.package\]/,/^\[/{s/^version = "([^"]+)"/\1/p}' "$(_ws)/Cargo.toml"; }

# True if ~/.local/bin/NAME (default aodm) is the one adx installed (unchanged since).
_ours() {
  local n="${1:-aodm}"
  [ -f "$(_marker "$n")" ] && [ -f "$(_bin "$n")" ] && [ "$(sha256sum "$(_bin "$n")" | cut -d' ' -f1)" = "$(cat "$(_marker "$n")")" ]
}

mod_status() {
  local b; b="$(_bin)"
  [ -x "$b" ] || { echo "aodm not installed (adx devices install)"; return 1; }
  local v; v="$("$b" --version 2>/dev/null || true)"
  echo "aodm: $b ($v)"
  if ! _ours; then echo "  not installed by adx (or changed since): adx devices install will not replace it"; return 1; fi
  case "$v" in "aodm $(_ver) "*) ;; *) echo "  version differs from tools/aodm ($(_ver)): adx devices install"; return 1 ;; esac
  # Built from the current commit? (dirty state ignored: any doc edit would mark it stale)
  local head; head="$(git -C "$ADX_ROOT" rev-parse --short=7 HEAD 2>/dev/null || true)"
  case "$v" in *"g$head"*|*"($head"*) ;; *) echo "  built from another commit than HEAD ($head): adx devices install"; return 1 ;; esac
  if [ -n "$(find "$(_ws)" -path "$(_ws)/target" -prune -o -type f -newer "$b" -print -quit)" ]; then
    echo "  stale: tools/aodm changed since this build (adx devices install)"; return 1
  fi
  local g; g="$(_bin aod-gui)"
  if [ ! -e "$g" ]; then echo "  window aod-gui not installed (adx devices install)"; return 1; fi
  if ! _ours aod-gui; then
    echo "  $g exists but was not installed by adx: move it away (or ADX_FORCE=1 adx devices install)"; return 1
  fi
  case "$("$g" --version 2>/dev/null)" in "aod-gui ${v#aodm }") ;; *) echo "  aod-gui is another build than aodm (adx devices install)"; return 1 ;; esac
  echo "aod-gui: $g (menu: Phone Images)"
  # Loaded at run time (the build needs no -dev packages); a warning, not a failure.
  # ldconfig lives in /sbin, often not on a user's PATH. Read whole (grep -q under
  # pipefail would fail on ldconfig's SIGPIPE).
  local lib libs; libs="$(ldconfig -p 2>/dev/null || /sbin/ldconfig -p 2>/dev/null || true)"
  for lib in libxkbcommon.so.0 libEGL.so.1; do
    grep -qF "$lib" <<<"$libs" || echo "  warning: $lib not found: the window may not open (install it with your package manager)"
  done
}

# Release build (locked, pinned target dir), then install atomically.
mod_install() {
  local cargo="$ADX_CARGO_HOME/bin/cargo" out target="$ADX_ROOT/tools/aodm/target"
  [ -x "$cargo" ] || adx_die "cargo not found (adx rust install)"
  adx_have cc || adx_die "no C linker (cc): install build-essential (see: adx system status)"
  local n
  for n in $_BINS; do
    if [ -e "$(_bin "$n")" ] && ! _ours "$n" && [ "${ADX_FORCE:-0}" != 1 ]; then
      adx_die "$(_bin "$n") exists and was not installed by adx; move it away (or ADX_FORCE=1 adx devices install)"
    fi
  done
  adx_log "building aodm $(_ver) (release; the first build downloads about 300 crates and takes a few minutes)"
  for n in $_BINS; do rm -f "$target/release/$n"; done   # never install a stale binary left by an earlier build
  # The caller's target/toolchain overrides must not change what gets built (pinned toolchain).
  (cd "$(_ws)" && env -u CARGO_BUILD_TARGET -u RUSTUP_TOOLCHAIN \
     CARGO_HOME="$ADX_CARGO_HOME" CARGO_TARGET_DIR="$target" ADX_ROOT="$ADX_ROOT" \
     "$cargo" build --release --locked)
  for n in $_BINS; do
    out="$target/release/$n"
    [ -x "$out" ] || adx_die "build produced no binary at $out"
  done
  mkdir -p "$(dirname "$(_bin)")" "$(dirname "$(_marker)")"
  # Two steps: stage every binary first, so a failure (disk full) changes nothing.
  for n in $_BINS; do install -m755 "$target/release/$n" "$(_bin "$n").new"; done
  for n in $_BINS; do
    mv -f "$(_bin "$n").new" "$(_bin "$n")"
    sha256sum "$(_bin "$n")" | cut -d' ' -f1 > "$(_marker "$n")"
  done
  adx_log "installed $(_bin) ($("$(_bin)" --version)) and $(_bin aod-gui)"
  _desktop
}

# True if the menu entry is adx's (a user's own file is never replaced or removed).
_desktop_ours() { grep -qx 'X-Adx-Managed=true' "$(_desktop_file)" 2>/dev/null; }

# Menu entry for the window (Programming -> Phone Images).
_desktop() {
  local f; f="$(_desktop_file)"
  if [ -e "$f" ] && ! _desktop_ours; then adx_warn "$f is not adx's; menu entry left alone"; return 0; fi
  mkdir -p "$(dirname "$f")"
  # Staged under a hidden name that still ends in .desktop (the validator requires it).
  local tmp; tmp="$(dirname "$f")/.aod-gui.new.desktop"
  cat > "$tmp" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Phone Images
GenericName=Android phone image manager (aodm)
Comment=See every adx phone image with a live screen thumbnail; start, stop, save, clone
Icon=phone
Exec="$(_bin aod-gui)"
Terminal=false
Categories=Development;
Keywords=android;emulator;avd;adx;aodm;phone;
StartupWMClass=aod-gui
X-Adx-Managed=true
DESKTOP
  if adx_have desktop-file-validate && ! desktop-file-validate "$tmp"; then
    rm -f "$tmp"; adx_die "invalid desktop file for $f"
  fi
  mv -f "$tmp" "$f"
  if adx_have update-desktop-database; then update-desktop-database "$(dirname "$f")" 2>/dev/null || true; fi
  adx_log "installed menu entry $f"
}

# Removes only adx's binaries and menu entry; images, saves and the registry are never touched.
mod_remove() {
  local n
  for n in $_BINS; do
    if [ -e "$(_bin "$n")" ] && ! _ours "$n"; then
      adx_die "$(_bin "$n") was not installed by adx (or changed since); not removing it"
    fi
  done
  for n in $_BINS; do rm -f "$(_bin "$n")" "$(_marker "$n")"; done
  if _desktop_ours; then rm -f "$(_desktop_file)"; fi
  adx_log "removed $_BINS and the menu entry (images, saves and $ADX_AOD_HOME untouched)"
}

# Pass-through: adx devices run <aodm args...>   e.g. adx devices run list
mod_run() {
  [ -x "$(_bin)" ] || adx_die "aodm not installed (adx devices install)"
  "$(_bin)" "$@"
}
