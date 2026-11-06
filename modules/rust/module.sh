MOD_DESC="Rust toolchain at user level via rustup (pinned rustup-init, SHA-256 checked, no sudo)"

_rustup() { CARGO_HOME="$ADX_CARGO_HOME" "$ADX_CARGO_HOME/bin/rustup" "$@"; }
_tc()     { echo "$ADX_RUST_TOOLCHAIN-x86_64-unknown-linux-gnu"; }

mod_status() {
  [ -x "$ADX_CARGO_HOME/bin/rustup" ] || { echo "rustup missing in $ADX_CARGO_HOME (adx rust install)"; return 1; }
  if ! _rustup toolchain list 2>/dev/null | grep -q "^$(_tc)"; then
    echo "toolchain $ADX_RUST_TOOLCHAIN missing (adx rust install)"; return 1
  fi
  local c rc=0
  for c in clippy rustfmt; do
    _rustup component list --installed --toolchain "$ADX_RUST_TOOLCHAIN" 2>/dev/null | grep -q "^$c" || { echo "component $c missing"; rc=1; }
  done
  echo "rust: $("$ADX_CARGO_HOME/bin/rustc" "+$ADX_RUST_TOOLCHAIN" --version 2>/dev/null) (cargo: $ADX_CARGO_HOME/bin/cargo)"
  return $rc
}

# Reuses an existing rustup; otherwise installs it from the pinned rustup-init.
mod_install() {
  if [ ! -x "$ADX_CARGO_HOME/bin/rustup" ]; then
    local init="$ADX_DOWNLOADS/rustup-init-$ADX_RUSTUP_VERSION"
    adx_download "https://static.rust-lang.org/rustup/archive/$ADX_RUSTUP_VERSION/x86_64-unknown-linux-gnu/rustup-init" "$init"
    adx_sha256_check "$init" "$ADX_RUSTUP_SHA256"
    chmod +x "$init"
    CARGO_HOME="$ADX_CARGO_HOME" "$init" -y --no-modify-path --profile minimal \
      --default-toolchain "$ADX_RUST_TOOLCHAIN" -c clippy -c rustfmt
  fi
  _rustup toolchain install --no-self-update "$ADX_RUST_TOOLCHAIN" --profile minimal -c clippy -c rustfmt
  adx_log "rust $ADX_RUST_TOOLCHAIN ready (cargo: $ADX_CARGO_HOME/bin/cargo; not added to PATH, modules use the full path)"
}
