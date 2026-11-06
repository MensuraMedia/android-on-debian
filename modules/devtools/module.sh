MOD_DESC="Developer tools for adx check: ShellCheck and cargo-deny (pinned, SHA-256) and a pre-commit hook"

_opt() { echo "$HOME/.local/opt/$1-$2"; }

# Download a pinned release tarball, verify, unpack into ~/.local/opt/<name>-<ver>, link the binary.
#   _get <name> <version> <url> <sha256> <path of binary inside the archive>
_get() {
  local name="$1" ver="$2" url="$3" sha="$4" inner="$5" file tmp link="$HOME/.local/bin/$1"
  if [ -e "$link" ] && [ ! -L "$link" ]; then
    adx_warn "$link is your own file, not replacing it; adx check uses the pinned copy in $(_opt "$name" "$ver")"
  fi
  if [ -x "$(_opt "$name" "$ver")/$name" ]; then adx_log "$name $ver already installed"; _link "$name" "$ver"; return 0; fi
  file="$ADX_DOWNLOADS/$(basename "$url")"
  adx_download "$url" "$file"
  adx_sha256_check "$file" "$sha"
  tmp="$(mktemp -d)"
  if ! tar -xf "$file" -C "$tmp"; then rm -rf "$tmp"; adx_die "cannot unpack $file"; fi
  mkdir -p "$(_opt "$name" "$ver")"
  install -m755 "$tmp/$inner" "$(_opt "$name" "$ver")/$name"
  rm -rf "$tmp"
  _link "$name" "$ver"
  adx_log "$name $ver installed ($(_opt "$name" "$ver"))"
}

# Link into ~/.local/bin unless the user has their own (non-symlink) file there.
_link() {
  local link="$HOME/.local/bin/$1"
  mkdir -p "$HOME/.local/bin"
  if [ -e "$link" ] && [ ! -L "$link" ]; then return 0; fi
  ln -sfn "$(_opt "$1" "$2")/$1" "$link"
}

_HOOK_MARK="# adx-devtools-hook v1"
_hook_path() { git -C "$ADX_ROOT" rev-parse --path-format=absolute --git-path hooks/pre-commit; }

mod_status() {
  local rc=0
  if [ -x "$(_opt shellcheck "$ADX_SHELLCHECK_VERSION")/shellcheck" ]; then echo "shellcheck $ADX_SHELLCHECK_VERSION: ok"
  else echo "shellcheck $ADX_SHELLCHECK_VERSION: missing (adx devtools install)"; rc=1; fi
  if [ -x "$(_opt cargo-deny "$ADX_CARGO_DENY_VERSION")/cargo-deny" ]; then echo "cargo-deny $ADX_CARGO_DENY_VERSION: ok"
  else echo "cargo-deny $ADX_CARGO_DENY_VERSION: missing (adx devtools install)"; rc=1; fi
  if grep -qxF "$_HOOK_MARK" "$(_hook_path)" 2>/dev/null; then
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
  local h; h="$(_hook_path)"
  local ours=false; grep -qxF "$_HOOK_MARK" "$h" 2>/dev/null && ours=true
  if [ "${1:-}" = remove ]; then
    [ -e "$h" ] || { adx_log "no pre-commit hook"; return 0; }
    [ "$ours" = true ] || adx_die "$h was not installed by adx; not removing it"
    rm -f "$h"; adx_log "removed $h"; return 0
  fi
  [ ! -e "$h" ] || [ "$ours" = true ] || adx_die "$h exists and isn't ours; not overwriting"
  mkdir -p "$(dirname "$h")"
  {
    echo '#!/usr/bin/env bash'
    echo "$_HOOK_MARK"
    echo '# Runs the adx gate on exactly what is staged. Skip once: git commit --no-verify'
    printf 'root=%q\n' "$ADX_ROOT"
    echo '# No-op on a checkout without adx check (e.g. after rolling back).'
    echo '[ -f "$root/lib/check.sh" ] || exit 0'
    echo 'exec "$root/adx" check --staged'
  } > "$h"
  chmod +x "$h"
  adx_log "installed $h"
}
