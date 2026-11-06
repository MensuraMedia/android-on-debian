MOD_DESC="mobile-mcp MCP server registered in Claude Code (drives emulator/device)"

_pkg() { echo "@mobilenext/mobile-mcp@$ADX_MOBILE_MCP_VERSION"; }

mod_status() {
  adx_have claude || { echo "claude CLI not found"; return 1; }
  adx_have npx    || { echo "npx not found (install Node.js)"; return 1; }
  if claude mcp get mobile-mcp >/dev/null 2>&1; then
    claude mcp get mobile-mcp 2>/dev/null | sed -n '1,8p'
  else
    echo "mobile-mcp not registered"; return 1
  fi
}

mod_install() {
  adx_have npx || adx_die "Node.js/npx required"
  adx_log "prefetching $(_pkg)"
  npm cache add "$(_pkg)" >/dev/null
  claude mcp add -s "$ADX_MCP_SCOPE" mobile-mcp -- npx -y "$(_pkg)"
  adx_log "registered mobile-mcp (scope: $ADX_MCP_SCOPE); restart Claude Code to load it"
}

mod_remove() { claude mcp remove -s "$ADX_MCP_SCOPE" mobile-mcp; }
