MOD_DESC="Node.js LTS at user level for mobile-mcp (nodejs.org tarball, SHA-256 checked, no sudo)"

_dir()  { echo "$HOME/.local/opt/node-v$ADX_NODE_VERSION-linux-x64"; }
_link() { echo "$HOME/.local/opt/node"; }
_major() { "$1" --version 2>/dev/null | sed -E 's/^v([0-9]+).*/\1/'; }

mod_status() {
  local n; n="$(command -v node || true)"
  if [ -z "$n" ]; then
    echo "node not on PATH (adx node install)"; return 1
  fi
  echo "node: $n $(node --version) (npx: $(command -v npx || echo missing))"
  [ "$(_major "$n")" -ge "$ADX_NODE_MIN_MAJOR" ] || { echo "node older than v$ADX_NODE_MIN_MAJOR (adx node install)"; return 1; }
  adx_have npx || return 1
}

mod_install() {
  local base="https://nodejs.org/dist/v$ADX_NODE_VERSION" tarball
  tarball="node-v$ADX_NODE_VERSION-linux-x64.tar.xz"
  if [ ! -x "$(_dir)/bin/npx" ]; then
    adx_log "downloading Node.js v$ADX_NODE_VERSION"
    adx_download "$base/$tarball" "$ADX_DOWNLOADS/$tarball"
    adx_sha256_check "$ADX_DOWNLOADS/$tarball" "$ADX_NODE_SHA256"
    mkdir -p "$HOME/.local/opt"
    local tmp; tmp="$(mktemp -d "$HOME/.local/opt/.node-XXXXXX")"
    tar -xJf "$ADX_DOWNLOADS/$tarball" -C "$tmp"
    rm -rf "$(_dir)"
    mv "$tmp/node-v$ADX_NODE_VERSION-linux-x64" "$(_dir)"
    rm -rf "$tmp"
  fi
  ln -sfn "$(_dir)" "$(_link)"
  mkdir -p "$HOME/.local/bin"
  local b; for b in node npm npx; do ln -sfn "$(_link)/bin/$b" "$HOME/.local/bin/$b"; done
  case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) adx_warn "$HOME/.local/bin is not on PATH in this shell; log out and in (Debian/Ubuntu ~/.profile adds it when it exists)" ;; esac
  adx_log "Node.js v$ADX_NODE_VERSION at $(_dir), linked into ~/.local/bin"
}
