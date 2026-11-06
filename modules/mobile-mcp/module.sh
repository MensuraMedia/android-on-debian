MOD_DESC="mobile-mcp MCP server registered in Claude Code (drives emulator/device)"

_pkg() { echo "@mobilenext/mobile-mcp@$ADX_MOBILE_MCP_VERSION"; }

mod_status() {
  adx_have claude || { echo "claude CLI not found"; return 1; }
  adx_have npx    || { echo "npx not found (install Node.js)"; return 1; }
  local info
  if ! info="$(claude mcp get mobile-mcp 2>/dev/null)"; then
    echo "mobile-mcp not registered"; return 1
  fi
  sed -n '1,8p' <<<"$info"
  grep -qF "$(_pkg)" <<<"$info" || { echo "registered, but not with $(_pkg) (adx mobile-mcp install)"; return 1; }
}

mod_install() {
  adx_have claude || adx_die "Claude Code CLI not found; install it (https://claude.com/claude-code), then rerun"
  adx_have npx || adx_die "Node.js/npx required (adx node install)"
  adx_log "prefetching $(_pkg)"
  npm cache add "$(_pkg)" >/dev/null
  claude mcp remove -s "$ADX_MCP_SCOPE" mobile-mcp >/dev/null 2>&1 || true   # re-add: version bumps
  claude mcp add -s "$ADX_MCP_SCOPE" mobile-mcp -- npx -y "$(_pkg)"
  adx_log "registered mobile-mcp (scope: $ADX_MCP_SCOPE); restart Claude Code to load it"
}

mod_remove() { claude mcp remove -s "$ADX_MCP_SCOPE" mobile-mcp; }
