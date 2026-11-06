MOD_DESC="Official Android Emulator + AVD (KVM, headless by default)"

_emu()  { "$ANDROID_HOME/emulator/emulator" "$@"; }
_adb()  { "$ANDROID_HOME/platform-tools/adb" "$@"; }
_avdm() { JAVA_HOME="$ADX_JAVA_HOME" "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" "$@"; }
_log()  { echo "$ADX_DOWNLOADS/emulator-$ADX_AVD_NAME.log"; }

mod_status() {
  local rc=0
  [ -x "$ANDROID_HOME/emulator/emulator" ] && echo "emulator: $(_emu -version 2>/dev/null | head -1)" || { echo "emulator package missing"; rc=1; }
  [ -d "$ANDROID_HOME/${ADX_AVD_IMAGE//;//}" ] && echo "image ok: $ADX_AVD_IMAGE" || { echo "image missing: $ADX_AVD_IMAGE"; rc=1; }
  _emu -list-avds 2>/dev/null | grep -qx "$ADX_AVD_NAME" && echo "avd ok: $ADX_AVD_NAME" || { echo "avd missing: $ADX_AVD_NAME"; rc=1; }
  [ -r /dev/kvm ] && [ -w /dev/kvm ] && echo "kvm: accessible" || { echo "kvm: /dev/kvm not accessible (sudo usermod -aG kvm \$USER)"; rc=1; }
  _adb devices 2>/dev/null | grep -q '^emulator-' && echo "running: yes" || echo "running: no"
  return $rc
}

mod_install() {
  JAVA_HOME="$ADX_JAVA_HOME" "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --install emulator "$ADX_AVD_IMAGE"
  mod_create
}

mod_create() {
  if _emu -list-avds 2>/dev/null | grep -qx "$ADX_AVD_NAME"; then
    adx_log "avd $ADX_AVD_NAME exists"; return 0
  fi
  echo no | _avdm create avd -n "$ADX_AVD_NAME" -k "$ADX_AVD_IMAGE" -d "$ADX_AVD_DEVICE"
  adx_log "created avd $ADX_AVD_NAME"
}

# adx emulator start [--window]
mod_start() {
  local flags=(-avd "$ADX_AVD_NAME" -gpu "$ADX_EMU_GPU" -no-boot-anim)
  if [ "${1:-}" != "--window" ] && [ "$ADX_EMU_HEADLESS" = true ]; then flags+=(-no-window -no-audio); fi
  if _adb devices | grep -q '^emulator-'; then adx_log "an emulator is already running"; return 0; fi
  mkdir -p "$ADX_DOWNLOADS"
  adx_log "starting $ADX_AVD_NAME (log: $(_log))"
  local t0=$SECONDS
  nohup "$ANDROID_HOME/emulator/emulator" "${flags[@]}" >"$(_log)" 2>&1 &
  timeout "$ADX_EMU_BOOT_TIMEOUT" "$ANDROID_HOME/platform-tools/adb" wait-for-device \
    || adx_die "no device after ${ADX_EMU_BOOT_TIMEOUT}s — see $(_log)"
  until [ "$(_adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do
    sleep 2
    [ $((SECONDS - t0)) -ge "$ADX_EMU_BOOT_TIMEOUT" ] && adx_die "boot timed out — see $(_log)"
  done
  adx_log "booted in ~$((SECONDS - t0))s: $(_adb devices | grep '^emulator-' | cut -f1)"
}

mod_stop() {
  local s
  for s in $(_adb devices | awk '/^emulator-/{print $1}'); do _adb -s "$s" emu kill; done
  adx_log "emulator stopped"
}

mod_accel() { _emu -accel-check; }

# Save a screenshot: adx emulator screenshot [file.png]
mod_screenshot() {
  local out="${1:-$ADX_DOWNLOADS/screen-$(date +%Y%m%d-%H%M%S).png}"
  _adb exec-out screencap -p > "$out" && adx_log "saved $out"
}
