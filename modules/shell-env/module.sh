MOD_DESC="ANDROID_HOME, JAVA_HOME and SDK tools on PATH via a managed ~/.bashrc block"

ADX_BASHRC="${ADX_BASHRC:-$HOME/.bashrc}"

mod_status() {
  if adx_block_present "$ADX_BASHRC" shell-env; then
    echo "managed block present in $ADX_BASHRC"
  else
    echo "no adx block in $ADX_BASHRC"; return 1
  fi
}

# The block only sources lib/shell-env.sh, so changing config never
# requires touching ~/.bashrc again.
mod_install() {
  adx_block_write "$ADX_BASHRC" shell-env \
"[ -f \"$ADX_ROOT/lib/shell-env.sh\" ] && . \"$ADX_ROOT/lib/shell-env.sh\""
  adx_log "added block to $ADX_BASHRC — open a new terminal or: . $ADX_BASHRC"
}

mod_remove() {
  adx_block_remove "$ADX_BASHRC" shell-env
  adx_log "removed block from $ADX_BASHRC"
}

# Print the variables a new shell will get.
mod_show() {
  ( . "$ADX_ROOT/lib/shell-env.sh"; env | grep -E '^(ANDROID_HOME|JAVA_HOME|ADX_ROOT)=' ; echo "PATH=$PATH" | tr ':' '\n' | grep -E 'Sdk|adx|android-on-debian' )
}
