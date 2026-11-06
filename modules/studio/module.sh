MOD_DESC="Android Studio (dormant: installed locally, not part of the default workflow)"

mod_status() {
  local f="$ADX_STUDIO_HOME/product-info.json"
  if [ -f "$f" ]; then
    echo "installed: $ADX_STUDIO_HOME ($(jq -r '.version' "$f"))"
  else
    echo "not installed at $ADX_STUDIO_HOME"; return 1
  fi
}

mod_install() {
  adx_warn "Android Studio is installed manually; see docs/android-studio.md"
  return 1
}

mod_launch() {
  [ -x "$ADX_STUDIO_HOME/bin/studio" ] || adx_die "not installed"
  nohup "$ADX_STUDIO_HOME/bin/studio" "$@" >/dev/null 2>&1 &
  adx_log "launched Android Studio"
}
