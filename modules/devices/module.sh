MOD_DESC="Phone image manager aodm (Rust): build with the pinned toolchain, install to ~/.local/bin"

_ws()     { echo "$ADX_ROOT/tools/aodm"; }
_bin()    { echo "$HOME/.local/bin/aodm"; }
# Marker proving the installed binary is adx's: its SHA-256, written at install.
_marker() { echo "$ADX_LOG_DIR/../aodm.installed"; }
_ver()    { sed -nE '/^\[workspace.package\]/,/^\[/{s/^version = "([^"]+)"/\1/p}' "$(_ws)/Cargo.toml"; }

# True if ~/.local/bin/aodm is the one adx installed (unchanged since).
_ours() {
  [ -f "$(_marker)" ] && [ -f "$(_bin)" ] && [ "$(sha256sum "$(_bin)" | cut -d' ' -f1)" = "$(cat "$(_marker)")" ]
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
}

# Release build (locked, pinned target dir), then install atomically.
mod_install() {
  local cargo="$ADX_CARGO_HOME/bin/cargo" out target="$ADX_ROOT/tools/aodm/target"
  [ -x "$cargo" ] || adx_die "cargo not found (adx rust install)"
  adx_have cc || adx_die "no C linker (cc): install build-essential (see: adx system status)"
  if [ -e "$(_bin)" ] && ! _ours && [ "${ADX_FORCE:-0}" != 1 ]; then
    adx_die "$(_bin) exists and was not installed by adx; move it away (or ADX_FORCE=1 adx devices install)"
  fi
  adx_log "building aodm $(_ver) (release; first build downloads crates and takes about a minute)"
  out="$target/release/aodm"
  rm -f "$out"   # never install a stale binary left by an earlier build
  # The caller's target/toolchain overrides must not change what gets built (pinned toolchain).
  (cd "$(_ws)" && env -u CARGO_BUILD_TARGET -u RUSTUP_TOOLCHAIN \
     CARGO_HOME="$ADX_CARGO_HOME" CARGO_TARGET_DIR="$target" ADX_ROOT="$ADX_ROOT" \
     "$cargo" build --release --locked)
  [ -x "$out" ] || adx_die "build produced no binary at $out"
  mkdir -p "$(dirname "$(_bin)")" "$(dirname "$(_marker)")"
  install -m755 "$out" "$(_bin).new"
  mv -f "$(_bin).new" "$(_bin)"
  sha256sum "$(_bin)" | cut -d' ' -f1 > "$(_marker)"
  adx_log "installed $(_bin) ($("$(_bin)" --version))"
}

# Removes only adx's binary; images, saves and the registry are never touched.
mod_remove() {
  if [ ! -e "$(_bin)" ]; then adx_log "aodm not installed"; return 0; fi
  _ours || adx_die "$(_bin) was not installed by adx (or changed since); not removing it"
  rm -f "$(_bin)" "$(_marker)"
  adx_log "removed $(_bin) (images, saves and $ADX_AOD_HOME untouched)"
}

# Pass-through: adx devices run <aodm args...>   e.g. adx devices run list
mod_run() {
  [ -x "$(_bin)" ] || adx_die "aodm not installed (adx devices install)"
  "$(_bin)" "$@"
}
