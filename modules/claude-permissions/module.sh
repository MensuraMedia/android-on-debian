MOD_DESC="Claude Code permission allowlist for Android app projects (.claude/settings.json)"

_tpl() { echo "$ADX_ROOT/templates/claude/settings.json"; }

# Status of this framework folder's own settings.
mod_status() {
  local f="$ADX_ROOT/.claude/settings.json"
  if [ -f "$f" ] && jq -e '.permissions.allow | length > 0' "$f" >/dev/null 2>&1; then
    echo "allowlist present: $f"
  else
    echo "no allowlist at $f"; return 1
  fi
}

# Install into this framework folder.
mod_install() { mod_apply "$ADX_ROOT"; }

# Merge the template allowlist into <dir>/.claude/settings.json without
# dropping rules that are already there: adx claude-permissions apply <dir>
mod_apply() {
  local dir="${1:?usage: adx claude-permissions apply <project-dir>}" f
  f="$dir/.claude/settings.json"
  mkdir -p "$dir/.claude"
  if [ -f "$f" ]; then
    jq -s '.[0] * {permissions: {
              allow: ((.[0].permissions.allow // []) + .[1].permissions.allow | unique),
              ask:   ((.[0].permissions.ask   // []) + .[1].permissions.ask   | unique)}}' \
       "$f" "$(_tpl)" > "$f.tmp" || { rm -f "$f.tmp"; adx_die "cannot merge into $f (invalid JSON?)"; }
    mv "$f.tmp" "$f"
  else
    cp "$(_tpl)" "$f"
  fi
  adx_log "allowlist applied: $f"
}

mod_show() { jq . "$(_tpl)"; }
