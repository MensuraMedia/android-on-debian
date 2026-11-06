MOD_DESC="Android SDK: cmdline-tools, platform-tools, platform, build-tools"

_sdkm() { "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" "$@"; }

_packages() {
  echo "platform-tools platforms;$ADX_SDK_PLATFORM build-tools;$ADX_SDK_BUILD_TOOLS $ADX_SDK_EXTRA_PACKAGES"
}

mod_status() {
  local ok=0 p
  [ -x "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" ] || { echo "cmdline-tools missing in $ANDROID_HOME"; return 1; }
  for p in $(_packages); do
    if [ -d "$ANDROID_HOME/${p//;//}" ]; then echo "ok      $p"; else echo "missing $p"; ok=1; fi
  done
  return $ok
}

mod_install() {
  export JAVA_HOME="$ADX_JAVA_HOME"
  if [ ! -x "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" ]; then
    local zip="$ADX_DOWNLOADS/commandlinetools-linux-${ADX_CMDLINE_TOOLS_BUILD}_latest.zip" tmp
    adx_log "downloading cmdline-tools $ADX_CMDLINE_TOOLS_BUILD"
    adx_download "https://dl.google.com/android/repository/commandlinetools-linux-${ADX_CMDLINE_TOOLS_BUILD}_latest.zip" "$zip"
    adx_sha256_check "$zip" "$ADX_CMDLINE_TOOLS_SHA256"
    tmp="$(mktemp -d)"
    unzip -q "$zip" -d "$tmp"
    mkdir -p "$ANDROID_HOME/cmdline-tools"
    rm -rf "$ANDROID_HOME/cmdline-tools/latest"
    mv "$tmp/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
    rm -rf "$tmp"
  fi
  # sdkmanager prompts for licences on older builds; newer builds ignore this.
  yes | _sdkm --licenses >/dev/null 2>&1 || true
  # shellcheck disable=SC2046
  _sdkm --install $(_packages)
}

# Install arbitrary packages: adx sdk add "platforms;android-36"
mod_add() { export JAVA_HOME="$ADX_JAVA_HOME"; _sdkm --install "$@"; }

mod_list() { export JAVA_HOME="$ADX_JAVA_HOME"; _sdkm --list_installed 2>/dev/null; }

mod_update() { export JAVA_HOME="$ADX_JAVA_HOME"; _sdkm --update; }
