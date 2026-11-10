MOD_DESC="ANDROID_HOME, JAVA_HOME and SDK tools on PATH via a managed ~/.bashrc block"

ADX_BASHRC="${ADX_BASHRC:-$HOME/.bashrc}"
# adx for shells that never read ~/.bashrc's interactive part (Claude Code's tool shell,
# scripts, cron): a link in ~/.local/bin, which is on PATH there.
_link() { echo "$HOME/.local/bin/adx"; }

mod_status() {
  local rc=0
  if adx_block_present "$ADX_BASHRC" shell-env; then
    echo "managed block present in $ADX_BASHRC"
  else
    echo "no adx block in $ADX_BASHRC"; rc=1
  fi
  if [ "$(readlink "$(_link)" 2>/dev/null)" = "$ADX_ROOT/adx" ]; then
    echo "adx command: $(_link)"
  else
    echo "no adx link in ~/.local/bin (adx shell-env install)"; rc=1
  fi
  return "$rc"
}

# The block only sources lib/shell-env.sh, so changing config never
# requires touching ~/.bashrc again.
mod_install() {
  adx_block_write "$ADX_BASHRC" shell-env \
"[ -f \"$ADX_ROOT/lib/shell-env.sh\" ] && . \"$ADX_ROOT/lib/shell-env.sh\""
  adx_log "added block to $ADX_BASHRC — open a new terminal or: . $ADX_BASHRC"
  if [ -e "$(_link)" ] && [ ! -L "$(_link)" ]; then
    adx_warn "$(_link) exists and is not a link: left alone"
  else
    mkdir -p "$(dirname "$(_link)")"
    ln -sfn "$ADX_ROOT/adx" "$(_link)"
    adx_log "linked $(_link) -> $ADX_ROOT/adx"
  fi
}

mod_remove() {
  adx_block_remove "$ADX_BASHRC" shell-env
  if [ "$(readlink "$(_link)" 2>/dev/null)" = "$ADX_ROOT/adx" ]; then rm -f "$(_link)"; fi
  adx_log "removed block from $ADX_BASHRC"
}

# Print the variables a new shell will get.
mod_show() {
  ( . "$ADX_ROOT/lib/shell-env.sh"; env | grep -E '^(ANDROID_HOME|JAVA_HOME|ADX_ROOT)=' ; echo "PATH=$PATH" | tr ':' '\n' | grep -E 'Sdk|adx|android-on-debian' )
}
