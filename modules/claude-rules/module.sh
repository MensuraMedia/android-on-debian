MOD_DESC="Global Claude Code rules (~/.claude/CLAUDE.md block) and agents (~/.claude/agents/, e.g. adversarial-reviewer)"

_src()    { echo "$ADX_ROOT/config/claude-global-rules.md"; }
_target() { echo "${ADX_CLAUDE_GLOBAL_MD:-$HOME/.claude/CLAUDE.md}"; }
_agents_src() { echo "$ADX_ROOT/templates/claude/agents"; }
_agents_dst() { echo "$HOME/.claude/agents"; }
# HTML-comment markers: invisible in rendered Markdown, unlike adx_block_write's '#' lines.
_begin='<!-- >>> adx:claude-rules >>> -->'
_end='<!-- <<< adx:claude-rules <<< -->'

# The rules file with @@ADX_ROOT@@ / @@ADX_LOG_DIR@@ etc. filled in for this machine.
_rendered() { perl -pe 's/@@([A-Z0-9_]+)@@/exists $ENV{$1} ? $ENV{$1} : $&/ge' "$(_src)"; }

_current_block() {
  awk -v b="$_begin" -v e="$_end" '$0==b{f=1;next} $0==e{f=0} f' "$(_target)" 2>/dev/null
}

mod_status() {
  local t; t="$(_target)"
  grep -qxF "$_begin" "$t" 2>/dev/null || { echo "rules block missing in $t (adx claude-rules install)"; return 1; }
  if [ "$(_current_block)" = "$(_rendered)" ]; then
    echo "rules block up to date in $t"
  else
    echo "rules block in $t differs from $(_src) (adx claude-rules install to update)"; return 1
  fi
  local a rc=0
  for a in "$(_agents_src)"/*.md; do
    if cmp -s "$a" "$(_agents_dst)/$(basename "$a")"; then
      echo "agent up to date: $(basename "$a" .md)"
    else
      echo "agent missing or changed: $(basename "$a" .md) (adx claude-rules install)"; rc=1
    fi
  done
  return $rc
}

# Write or refresh the block; text outside the markers is kept.
mod_install() {
  local t tmp; t="$(readlink -f "$(_target)" 2>/dev/null || _target)"
  mkdir -p "$(dirname "$t")"; touch "$t"
  tmp="$(mktemp)"
  awk -v b="$_begin" -v e="$_end" '$0==b{f=1} !f{print} $0==e{f=0}' "$t" > "$tmp"
  { printf '%s\n' "$_begin"; _rendered; printf '%s\n' "$_end"; [ -s "$tmp" ] && { echo; cat "$tmp"; }; } > "$t.new"
  rm -f "$tmp"
  mv "$t.new" "$t"
  adx_log "installed global rules into $t"
  mkdir -p "$(_agents_dst)"
  local a d
  for a in "$(_agents_src)"/*.md; do
    d="$(_agents_dst)/$(basename "$a")"
    if [ -f "$d" ] && ! cmp -s "$a" "$d"; then cp "$d" "$d.bak"; adx_warn "kept your edited $(basename "$d") as $d.bak"; fi
    cp "$a" "$d"
  done
  adx_log "installed agents into $(_agents_dst): $(cd "$(_agents_src)" && ls ./*.md | sed 's|^\./||; s|\.md$||' | paste -sd ' ')"
}

mod_remove() {
  local t; t="$(_target)"
  [ -f "$t" ] || return 0
  awk -v b="$_begin" -v e="$_end" '$0==b{f=1} !f{print} $0==e{f=0}' "$t" > "$t.new" && mv "$t.new" "$t"
  local a; for a in "$(_agents_src)"/*.md; do rm -f "$(_agents_dst)/$(basename "$a")"; done
  adx_log "removed global rules block from $t and the agents adx installed"
}
