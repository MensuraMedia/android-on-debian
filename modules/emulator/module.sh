MOD_DESC="Official Android Emulator + AVD (KVM, headless by default; menu entry via: adx emulator desktop)"

_emu()  { "$ANDROID_HOME/emulator/emulator" "$@"; }
_adb()  { "$ANDROID_HOME/platform-tools/adb" "$@"; }
_avdm() { JAVA_HOME="$ADX_JAVA_HOME" "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" "$@"; }
_log()  { echo "$ADX_LOG_DIR/emulator-$ADX_AVD_NAME.log"; }
# PID of this AVD's emulator process, if one is running.
_pid()  { pgrep -f "qemu-system-[^ ]* .*-avd $ADX_AVD_NAME( |\$)" | head -1; }
# This AVD's adb serial if adb sees it (console port pinned by ADX_EMU_PORT).
_serial() { _adb devices 2>/dev/null | awk -v s="emulator-$ADX_EMU_PORT" '$1==s{print $1; exit}'; }
_desktop_file() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/applications/adx-emulator.desktop"; }

# Hybrid Intel/AMD + NVIDIA laptops (PRIME) are where -gpu host has crashed.
# Display controllers (PCI class 0x03xxxx) by vendor ID: 10de NVIDIA, 8086 Intel, 1002 AMD.
_hybrid_gpu() {
  local d nvidia=0 other=0
  for d in /sys/bus/pci/devices/*; do
    case "$(cat "$d/class" 2>/dev/null)" in
      0x03*) case "$(cat "$d/vendor" 2>/dev/null)" in
               0x10de) nvidia=1 ;;
               0x8086|0x1002) other=1 ;;
             esac ;;
    esac
  done
  [ "$nvidia" = 1 ] && [ "$other" = 1 ]
}

_notify() {
  if [ "${_ADX_NOTIFY:-false}" = true ] && adx_have notify-send; then
    notify-send -i phone "Android Emulator" "$*" || true
  fi
}

mod_status() {
  local rc=0
  [ -x "$ANDROID_HOME/emulator/emulator" ] && echo "emulator: $(_emu -version 2>/dev/null | head -1)" || { echo "emulator package missing"; rc=1; }
  [ -d "$ANDROID_HOME/${ADX_AVD_IMAGE//;//}" ] && echo "image ok: $ADX_AVD_IMAGE" || { echo "image missing: $ADX_AVD_IMAGE"; rc=1; }
  _emu -list-avds 2>/dev/null | grep -qx "$ADX_AVD_NAME" && echo "avd ok: $ADX_AVD_NAME" || { echo "avd missing: $ADX_AVD_NAME"; rc=1; }
  [ -r /dev/kvm ] && [ -w /dev/kvm ] && echo "kvm: accessible" || { echo "kvm: /dev/kvm not accessible (sudo usermod -aG kvm \$USER)"; rc=1; }
  echo "gpu: $ADX_EMU_GPU${ADX_EMU_ENV:+ (env: $ADX_EMU_ENV)}"
  if _hybrid_gpu && [ "$ADX_EMU_GPU" = host ]; then
    echo "  hint: hybrid Intel/AMD + NVIDIA GPUs detected and ADX_EMU_GPU=host; if the emulator"
    echo "        dies with GL 'null ctx' errors, run: adx emulator autoconfig (sets ADX_EMU_GPU=software)"
  fi
  local pid serial; pid="$(_pid)"; serial="$(_serial)"
  if [ -n "$pid" ] && [ -n "$serial" ]; then
    echo "running: yes ($serial, pid $pid, adb state: $(_adb -s "$serial" get-state 2>/dev/null || echo unknown))"
  elif [ -n "$pid" ]; then
    echo "running: process $pid alive but adb sees no device; run: adx emulator start (restarts the adb server)"
  elif [ -n "$serial" ]; then
    echo "running: adb lists $serial but no local emulator process (stale entry or another AVD)"
  else
    echo "running: no"
    if [ -s "$(_log)" ] && grep -qE 'FATAL|Segmentation|context was preempted|null ctx' "$(_log)"; then
      echo "  last run logged errors: adx logs emulator"
    fi
  fi
  if [ -f "$(_desktop_file)" ]; then echo "menu entry: $(_desktop_file)"; else echo "menu entry: not installed (adx emulator desktop)"; fi
  return $rc
}

mod_install() {
  JAVA_HOME="$ADX_JAVA_HOME" "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --install emulator "$ADX_AVD_IMAGE"
  mod_create
  mod_autoconfig
}

# Machine-specific emulator settings, written to config/local.env only when
# the user has not set them. Hybrid Intel/AMD + NVIDIA (PRIME) laptops:
#  - first cold boot segfaulted in libtcmalloc without NVIDIA offload (2026-11-05)
#  - -gpu host died within minutes with GL "null ctx" errors (2026-11-06)
mod_autoconfig() {
  if ! _hybrid_gpu; then adx_log "single GPU: keeping ADX_EMU_GPU=$ADX_EMU_GPU"; return 0; fi
  adx_local_set ADX_EMU_ENV "__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia __VK_LAYER_NV_optimus=NVIDIA_only" \
    "Hybrid GPU: run the emulator on NVIDIA (avoids first-boot libtcmalloc segfault)" \
    && adx_log "hybrid GPU: set ADX_EMU_ENV (NVIDIA offload) in config/local.env"
  adx_local_set ADX_EMU_GPU "software" \
    "Hybrid GPU: -gpu host crashed with GL 'null ctx' errors; software (Lavapipe) is stable" \
    && adx_log "hybrid GPU: set ADX_EMU_GPU=software in config/local.env"
  adx_log "emulator config: see config/local.env (applies from the next start)"
}

mod_create() {
  if _emu -list-avds 2>/dev/null | grep -qx "$ADX_AVD_NAME"; then
    adx_log "avd $ADX_AVD_NAME exists"; return 0
  fi
  echo no | _avdm create avd -n "$ADX_AVD_NAME" -k "$ADX_AVD_IMAGE" -d "$ADX_AVD_DEVICE"
  adx_log "created avd $ADX_AVD_NAME"
}

# adx emulator start [--window] [--wipe] [--notify]
#   --window  show the emulator window (default follows ADX_EMU_HEADLESS)
#   --wipe    factory-reset the AVD's user data (fixes a device left
#             "unauthorized" by a crashed first boot)
#   --notify  desktop notification on success/failure (used by the menu entry)
mod_start() {
  local flags=(-avd "$ADX_AVD_NAME" -gpu "$ADX_EMU_GPU" -no-boot-anim -port "$ADX_EMU_PORT") window=false a
  for a in "$@"; do
    case "$a" in
      --window) window=true ;;
      --wipe)   flags+=(-wipe-data) ;;
      --notify) _ADX_NOTIFY=true ;;
      *)        adx_die "unknown option '$a' (usage: adx emulator start [--window] [--wipe] [--notify])" ;;
    esac
  done
  # adx_die exits this module subshell; report failures to the desktop too.
  trap 'rc=$?; if [ $rc -ne 0 ]; then [ -n "${_ADX_EMU_PID:-}" ] && kill "$_ADX_EMU_PID" 2>/dev/null; _notify "Failed to start (exit $rc). See: adx logs emulator"; fi' EXIT
  if [ "$window" = false ] && [ "$ADX_EMU_HEADLESS" = true ]; then flags+=(-no-window -no-audio); fi

  if [ -n "$(_pid)" ]; then
    if [ -n "$(_serial)" ]; then
      adx_log "an emulator is already running ($(_serial))"; _notify "Already running"; return 0
    fi
    # Seen 2026-11-06: emulator alive, adb lost it. Restarting the adb server reattaches.
    adx_warn "emulator process $(_pid) is running but adb lost it; restarting the adb server"
    _adb kill-server >/dev/null 2>&1 || true
    _adb start-server >/dev/null 2>&1
    local i; for i in $(seq 1 15); do [ -n "$(_serial)" ] && break; sleep 1; done
    [ -n "$(_serial)" ] || adx_die "adb still sees no device; run: adx emulator stop, then start again"
    adx_log "reattached to $(_serial)"; _notify "Reattached to running emulator"; return 0
  fi
  mkdir -p "$ADX_LOG_DIR"
  adx_log_shift "$(_log)"   # keep the previous run's log (crash evidence) as .1
  adx_log "starting $ADX_AVD_NAME (gpu: $ADX_EMU_GPU, window: $window, log: $(_log))"
  local t0=$SECONDS pid
  # ADX_EMU_ENV is a space-separated VAR=value list (no spaces inside values).
  local -a ev=(); read -ra ev <<<"$ADX_EMU_ENV"
  for a in "${ev[@]}"; do [[ $a == [A-Za-z_]*=* ]] || adx_die "ADX_EMU_ENV: '$a' is not VAR=value"; done
  nohup env "${ev[@]}" "$ANDROID_HOME/emulator/emulator" "${flags[@]}" >"$(_log)" 2>&1 &
  pid=$!
  _ADX_EMU_PID=$pid   # killed by the EXIT trap if start fails
  # Poll instead of `adb wait-for-device` so a crashed emulator fails fast.
  until [ "$(_adb -s "emulator-$ADX_EMU_PORT" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do
    kill -0 "$pid" 2>/dev/null || adx_die "emulator exited during boot — see $(_log); retry, or use --wipe if it keeps failing"
    if [ $((SECONDS - t0)) -ge 60 ] && _adb -s "emulator-$ADX_EMU_PORT" get-state 2>&1 | grep -q unauthorized; then
      adx_die "device is 'unauthorized' (user data from a crashed boot?) — run: adx emulator stop && adx emulator start --wipe"
    fi
    [ $((SECONDS - t0)) -ge "$ADX_EMU_BOOT_TIMEOUT" ] && adx_die "boot timed out — see $(_log)"
    sleep 2
  done
  adx_log "booted in ~$((SECONDS - t0))s: $(_serial) (pid $pid)"
  _notify "Booted in ~$((SECONDS - t0)) s"
}

# adx emulator stop [--notify]
mod_stop() {
  [ "${1:-}" = --notify ] && _ADX_NOTIFY=true
  local s pid i
  # Only this AVD's emulator: other emulators (aodm images, spin-offs) keep running.
  _adb -s "emulator-$ADX_EMU_PORT" emu kill >/dev/null 2>&1 || true
  pid="$(_pid)"
  if [ -n "$pid" ]; then
    for i in $(seq 1 15); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
    if kill -0 "$pid" 2>/dev/null; then
      adx_warn "emulator $pid did not exit after 'emu kill'; sending SIGTERM"
      kill "$pid" 2>/dev/null || true
    fi
  fi
  adx_log "emulator stopped"
  _notify "Stopped"
}

# Desktop menu entry (Programming -> Android Emulator) with Stop and
# Start-with-wiped-data actions: adx emulator desktop [remove]
mod_desktop() {
  local f; f="$(_desktop_file)"
  if [ "${1:-}" = remove ]; then
    rm -f "$f"; adx_log "removed $f"; return 0
  fi
  mkdir -p "$(dirname "$f")"
  cat > "$f" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Android Emulator
GenericName=Android Emulator ($ADX_AVD_NAME)
Comment=Start the adx Android emulator in a window
Icon=phone
Exec="$ADX_ROOT/adx" emulator start --window --notify
Terminal=false
Categories=Development;
Keywords=android;emulator;avd;adx;
StartupNotify=false
Actions=stop;wipe;

[Desktop Action stop]
Name=Stop Emulator
Exec="$ADX_ROOT/adx" emulator stop --notify

[Desktop Action wipe]
Name=Start with Wiped Data
Exec="$ADX_ROOT/adx" emulator start --window --wipe --notify
DESKTOP
  if adx_have desktop-file-validate; then desktop-file-validate "$f" || adx_die "invalid desktop file $f"; fi
  if adx_have update-desktop-database; then update-desktop-database "$(dirname "$f")" 2>/dev/null || true; fi
  adx_log "installed menu entry $f"
}

mod_accel() { _emu -accel-check; }

# Save a screenshot: adx emulator screenshot [file.png]
mod_screenshot() {
  local out="${1:-$ADX_DOWNLOADS/screen-$(date +%Y%m%d-%H%M%S).png}"
  mkdir -p "$(dirname "$out")"
  _adb -s "emulator-$ADX_EMU_PORT" exec-out screencap -p > "$out" || adx_die "screenshot failed (is $ADX_AVD_NAME running? adx emulator status)"
  adx_log "saved $out"
}
