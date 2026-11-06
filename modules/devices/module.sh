MOD_DESC="Phone image manager aodm (Rust): build with the pinned toolchain, install to ~/.local/bin"

_ws()  { echo "$ADX_ROOT/tools/aodm"; }
_bin() { echo "$HOME/.local/bin/aodm"; }
_ver() { sed -nE '/^\[workspace.package\]/,/^\[/{s/^version = "([^"]+)"/\1/p}' "$(_ws)/Cargo.toml"; }

# Newest source file under the workspace (excluding build output), as epoch seconds.
_src_mtime() {
  find "$(_ws)" -path "$(_ws)/target" -prune -o -type f -printf '%T@\n' | sort -n | tail -1 | cut -d. -f1
}

mod_status() {
  local b; b="$(_bin)"
  [ -x "$b" ] || { echo "aodm not installed (adx devices install)"; return 1; }
  local v; v="$("$b" --version 2>/dev/null)"
  echo "aodm: $b ($v)"
  case "$v" in "aodm $(_ver) "*) ;; *) echo "  version differs from tools/aodm ($(_ver)): adx devices install"; return 1 ;; esac
  if [ "$(_src_mtime)" -gt "$(stat -c %Y "$b")" ]; then
    echo "  stale: tools/aodm changed since this build (adx devices install)"; return 1
  fi
}

# Release build (locked), then install atomically.
mod_install() {
  local cargo="$ADX_CARGO_HOME/bin/cargo" out
  [ -x "$cargo" ] || adx_die "cargo not found (adx rust install)"
  adx_log "building aodm $(_ver) (release)"
  (cd "$(_ws)" && CARGO_HOME="$ADX_CARGO_HOME" ADX_ROOT="$ADX_ROOT" "$cargo" build --release --locked -q)
  out="$(_ws)/target/release/aodm"
  [ -x "$out" ] || adx_die "build produced no binary at $out"
  mkdir -p "$(dirname "$(_bin)")"
  install -m755 "$out" "$(_bin).new"
  mv -f "$(_bin).new" "$(_bin)"
  adx_log "installed $(_bin) ($("$(_bin)" --version))"
}

# Removes only the binary; images, saves and the registry are never touched.
mod_remove() {
  rm -f "$(_bin)"
  adx_log "removed $(_bin) (images, saves and $ADX_AOD_HOME untouched)"
}

# Pass-through: adx devices run <aodm args...>   e.g. adx devices run list
mod_run() {
  [ -x "$(_bin)" ] || adx_die "aodm not installed (adx devices install)"
  "$(_bin)" "$@"
}
