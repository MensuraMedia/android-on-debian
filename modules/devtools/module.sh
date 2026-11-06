MOD_DESC="Developer tools for adx check: ShellCheck and cargo-deny (pinned, SHA-256) and a pre-commit hook"

_opt() { echo "$HOME/.local/opt/$1-$2"; }

# Download a pinned release tarball, verify, unpack into ~/.local/opt/<name>-<ver>, link the binary.
#   _get <name> <version> <url> <sha256> <path of binary inside the archive>
_get() {
  local name="$1" ver="$2" url="$3" sha="$4" inner="$5" file tmp
  file="$ADX_DOWNLOADS/$(basename "$url")"
  adx_download "$url" "$file"
  adx_sha256_check "$file" "$sha"
  tmp="$(mktemp -d)"
  tar -xf "$file" -C "$tmp"
  mkdir -p "$(_opt "$name" "$ver")" "$HOME/.local/bin"
  install -m755 "$tmp/$inner" "$(_opt "$name" "$ver")/$name"
  rm -rf "$tmp"
  ln -sfn "$(_opt "$name" "$ver")/$name" "$HOME/.local/bin/$name"
  adx_log "$name $ver installed ($(_opt "$name" "$ver"))"
}

mod_status() {
  local rc=0
  if [ -x "$(_opt shellcheck "$ADX_SHELLCHECK_VERSION")/shellcheck" ]; then echo "shellcheck $ADX_SHELLCHECK_VERSION: ok"
  else echo "shellcheck $ADX_SHELLCHECK_VERSION: missing (adx devtools install)"; rc=1; fi
  if [ -x "$(_opt cargo-deny "$ADX_CARGO_DENY_VERSION")/cargo-deny" ]; then echo "cargo-deny $ADX_CARGO_DENY_VERSION: ok"
  else echo "cargo-deny $ADX_CARGO_DENY_VERSION: missing (adx devtools install)"; rc=1; fi
  if [ -f "$ADX_ROOT/.git/hooks/pre-commit" ] && grep -q 'adx check' "$ADX_ROOT/.git/hooks/pre-commit"; then
    echo "pre-commit hook: installed"
  else
    echo "pre-commit hook: not installed (optional: adx devtools hook)"
  fi
  return $rc
}

mod_install() {
  _get shellcheck "$ADX_SHELLCHECK_VERSION" \
    "https://github.com/koalaman/shellcheck/releases/download/v$ADX_SHELLCHECK_VERSION/shellcheck-v$ADX_SHELLCHECK_VERSION.linux.x86_64.tar.xz" \
    "$ADX_SHELLCHECK_SHA256" "shellcheck-v$ADX_SHELLCHECK_VERSION/shellcheck"
  _get cargo-deny "$ADX_CARGO_DENY_VERSION" \
    "https://github.com/EmbarkStudios/cargo-deny/releases/download/$ADX_CARGO_DENY_VERSION/cargo-deny-$ADX_CARGO_DENY_VERSION-x86_64-unknown-linux-musl.tar.gz" \
    "$ADX_CARGO_DENY_SHA256" "cargo-deny-$ADX_CARGO_DENY_VERSION-x86_64-unknown-linux-musl/cargo-deny"
}

# Optional git pre-commit hook running the fast gate on staged changes:
#   adx devtools hook [remove]
mod_hook() {
  local h="$ADX_ROOT/.git/hooks/pre-commit"
  if [ "${1:-}" = remove ]; then rm -f "$h"; adx_log "removed $h"; return 0; fi
  [ ! -e "$h" ] || grep -q 'adx check' "$h" || adx_die "$h exists and isn't ours; not overwriting"
  printf '#!/usr/bin/env bash\n# Installed by adx devtools hook. Skip once with: git commit --no-verify\n# No-op if this checkout of adx has no check command (e.g. after rolling back).\n[ -f "%s/lib/check.sh" ] || exit 0\nexec "%s/adx" check --staged\n' "$ADX_ROOT" "$ADX_ROOT" > "$h"
  chmod +x "$h"
  adx_log "installed $h"
}
