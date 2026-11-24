MOD_DESC="Android Studio (dormant: installed locally for when it's needed, adx studio launch)"
MOD_CHANGES="installs Android Studio in ~/.local/opt/android-studio; keeps the 1.6 GB tarball in ~/Downloads/adx; an install of another build is kept as android-studio.previous-<build>-<time>; no menu entry or PATH change"

_build() { jq -r '.buildNumber // empty' "$ADX_STUDIO_HOME/product-info.json" 2>/dev/null || true; }

mod_status() {
  local f="$ADX_STUDIO_HOME/product-info.json" b
  if [ -f "$f" ]; then
    b="$(_build)"
    echo "installed: $ADX_STUDIO_HOME ($(jq -r '.version' "$f"))"
    if [ "$b" != "$ADX_STUDIO_BUILD" ]; then
      echo "  build ${b:-unknown} differs from the pinned $ADX_STUDIO_BUILD ($ADX_STUDIO_VERSION): adx studio install"; return 1
    fi
  else
    echo "not installed at $ADX_STUDIO_HOME (adx studio install)"; return 1
  fi
}

# Official tarball, SHA-256 checked, unpacked to ADX_STUDIO_HOME. Nothing is deleted: an
# install of another build is kept as ADX_STUDIO_HOME.previous-<build>-<time>, and the
# staging folder is per build, so an interrupted run is overwritten by the next, not piled up.
# shortcut: pinned build only (no in-place IDE updates by adx), upgrade by bumping ADX_STUDIO_*.
mod_install() {
  if mod_status >/dev/null 2>&1; then adx_log "Android Studio $ADX_STUDIO_VERSION already installed"; return 0; fi
  local tgz stage old
  tgz="$ADX_DOWNLOADS/$(basename "$ADX_STUDIO_URL")"
  adx_log "downloading Android Studio $ADX_STUDIO_VERSION (1.6 GB, kept in $ADX_DOWNLOADS)"
  adx_download "$ADX_STUDIO_URL" "$tgz"
  adx_sha256_check "$tgz" "$ADX_STUDIO_SHA256"
  stage="$(dirname "$ADX_STUDIO_HOME")/.android-studio-$ADX_STUDIO_BUILD.staging"
  mkdir -p "$stage"
  tar -xzf "$tgz" -C "$stage"
  [ -x "$stage/android-studio/bin/studio" ] || [ -x "$stage/android-studio/bin/studio.sh" ] || adx_die "unexpected tarball layout in $tgz (staged in $stage)"
  if [ -e "$ADX_STUDIO_HOME" ]; then
    old="$(_build)"
    old="$ADX_STUDIO_HOME.previous-${old:-unknown}-$(date +%Y%m%d%H%M%S)"
    mv "$ADX_STUDIO_HOME" "$old"
    adx_log "kept the previous install as $old"
  fi
  mv "$stage/android-studio" "$ADX_STUDIO_HOME"
  rmdir "$stage" 2>/dev/null || adx_warn "staging folder $stage not empty; left in place"
  adx_log "installed $ADX_STUDIO_HOME ($(jq -r '.version' "$ADX_STUDIO_HOME/product-info.json" 2>/dev/null))"
}

mod_launch() {
  local bin="$ADX_STUDIO_HOME/bin/studio"
  [ -x "$bin" ] || bin="$ADX_STUDIO_HOME/bin/studio.sh"
  [ -x "$bin" ] || adx_die "not installed (adx studio install)"
  nohup "$bin" "$@" >>"$ADX_LOG_DIR/studio.log" 2>&1 &
  adx_log "launched Android Studio"
}
