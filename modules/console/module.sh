MOD_DESC="Local web console (adx console start): phone images, assets, apps, modules, skills, activity, settings"

_server()  { echo "$ADX_ROOT/tools/console/server.py"; }
_url()     { echo "http://127.0.0.1:$ADX_CONSOLE_PORT"; }
_pidfile() { echo "$ADX_LOG_DIR/console.pid"; }
_logfile() { echo "$ADX_LOG_DIR/console.log"; }
_desktop_file() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/applications/adx-console.desktop"; }
# App icon: one artwork for the menu entry (hicolor theme icon "adx-console"), and the
# window (the page favicon, which Chromium's --app window shows in the panel and Alt+Tab).
_icon_src()   { echo "$ADX_ROOT/tools/console/static/app-icon"; }
_icon_theme() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"; }
_ICON_SIZES="16 22 24 32 48 64 128 256 512"

# Our server answers /api/health with app "adx console" (another app may hold the port).
_running() { curl -fsS --max-time 2 "$(_url)/api/health" 2>/dev/null | grep -q '"adx console"'; }

# PID from the pid file, only if that process is still our server.
_pid() {
  local pid; pid="$(cat "$(_pidfile)" 2>/dev/null || true)"
  if [ -n "$pid" ] && tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -qF "$(_server)"; then
    echo "$pid"
  fi
}

mod_status() {
  local rc=0
  if adx_have python3; then echo "python3: $(python3 --version 2>&1)"; else echo "python3 missing"; rc=1; fi
  if [ -f "$(_desktop_file)" ]; then echo "menu entry: $(_desktop_file)"; else echo "menu entry missing (adx console install)"; rc=1; fi
  if ! cmp -s "$(_icon_theme)/48x48/apps/adx-console.png" "$(_icon_src)/48.png"; then
    echo "app icon missing or outdated in $(_icon_theme) (adx console install)"; rc=1
  fi
  if _running; then echo "running: $(_url) (pid $(_pid))"; else echo "not running (adx console start)"; fi
  return "$rc"
}

# Nothing to download: the server is Python standard library only. Installs the
# applications-menu entry (Development -> adx console).
mod_install() {
  adx_have python3 || adx_die "python3 not found (install it with your package manager)"
  _icons
  local f; f="$(_desktop_file)"
  mkdir -p "$(dirname "$f")"
  cat > "$f" <<DESKTOP
[Desktop Entry]
Type=Application
Name=adx console
GenericName=Android Dev Console
Comment=Phone images, apps, modules and skills of android-on-debian
Icon=adx-console
Exec="$ADX_ROOT/adx" console start
Terminal=false
Categories=Development;
Keywords=android;emulator;adx;aodm;console;
StartupNotify=false
StartupWMClass=adx-console
Actions=stop;

[Desktop Action stop]
Name=Stop the console
Exec="$ADX_ROOT/adx" console stop
DESKTOP
  if adx_have desktop-file-validate; then desktop-file-validate "$f" || adx_die "invalid desktop file $f"; fi
  if adx_have update-desktop-database; then update-desktop-database "$(dirname "$f")" 2>/dev/null || true; fi
  adx_log "installed menu entry $f (start: adx console start)"
}

# Install the app icon into the user's hicolor theme as "adx-console" (PNG per size + SVG).
_icons() {
  local s d theme; theme="$(_icon_theme)"
  for s in $_ICON_SIZES; do
    d="$theme/${s}x${s}/apps"
    mkdir -p "$d"
    install -m644 "$(_icon_src)/$s.png" "$d/adx-console.png"
  done
  mkdir -p "$theme/scalable/apps"
  install -m644 "$(_icon_src)/adx-console.svg" "$theme/scalable/apps/adx-console.svg"
  touch "$theme"   # icon caches and the desktop notice the change
  if adx_have gtk-update-icon-cache && [ -f "$theme/index.theme" ]; then
    gtk-update-icon-cache -q -t -f "$theme" 2>/dev/null || true
  fi
  adx_log "installed icon adx-console into $theme"
}

# Remove only icon files identical to adx's sources (a user's own adx-console icon stays).
_icons_remove() {
  local s f theme; theme="$(_icon_theme)"
  for s in $_ICON_SIZES; do
    f="$theme/${s}x${s}/apps/adx-console.png"
    if cmp -s "$f" "$(_icon_src)/$s.png"; then rm -f "$f"; fi
  done
  f="$theme/scalable/apps/adx-console.svg"
  if cmp -s "$f" "$(_icon_src)/adx-console.svg"; then rm -f "$f"; fi
}

mod_remove() {
  mod_stop
  _icons_remove
  rm -f "$(_desktop_file)"
  adx_log "removed $(_desktop_file)"
}

# Open the console in its own chromeless window (Chromium-family --app mode,
# isolated profile), else the default browser.
mod_open() {
  local b prof="$ADX_LOG_DIR/console-window"
  for b in google-chrome google-chrome-stable chromium chromium-browser brave-browser microsoft-edge; do
    if adx_have "$b"; then
      "$b" --app="$(_url)" --class=adx-console --user-data-dir="$prof" --window-size=1240,860 \
        --no-first-run --no-default-browser-check >/dev/null 2>&1 &
      return 0
    fi
  done
  xdg-open "$(_url)" >/dev/null 2>&1 || adx_warn "open $(_url) in a browser"
}

# adx console start [--no-window]
mod_start() {
  local window=true i
  if [ "${1:-}" = --no-window ]; then window=false; fi
  if ! _running; then
    adx_have python3 || adx_die "python3 not found"
    mkdir -p "$ADX_LOG_DIR"
    ADX_ROOT="$ADX_ROOT" ADX_APPS_DIR="$ADX_APPS_DIR" ADX_LOG_DIR="$ADX_LOG_DIR" \
      ANDROID_HOME="$ANDROID_HOME" ADX_CONSOLE_PORT="$ADX_CONSOLE_PORT" \
      ADX_CONSOLE_ASSETS_DIR="$ADX_CONSOLE_ASSETS_DIR" ADX_CONSOLE_UPLOAD_MAX_MB="$ADX_CONSOLE_UPLOAD_MAX_MB" \
      setsid python3 "$(_server)" >>"$(_logfile)" 2>&1 < /dev/null &
    echo "$!" > "$(_pidfile)"
    for i in $(seq 1 20); do
      if _running; then break; fi
      sleep 0.5
    done
    _running || adx_die "console did not start (port $ADX_CONSOLE_PORT busy?); see $(_logfile)"
    adx_log "console running at $(_url) (pid $(_pid); log $(_logfile))"
  else
    adx_log "console already running at $(_url)"
  fi
  if [ "$window" = true ]; then mod_open; fi
}

mod_stop() {
  local pid i; pid="$(_pid)"
  if [ -z "$pid" ]; then
    rm -f "$(_pidfile)"
    if _running; then adx_warn "a console runs at $(_url) but was not started from this pid file; stop it there"; fi
    adx_log "console not running"; return 0
  fi
  kill "$pid" 2>/dev/null || true
  for i in $(seq 1 10); do
    if ! kill -0 "$pid" 2>/dev/null; then break; fi
    sleep 0.5
  done
  rm -f "$(_pidfile)"
  adx_log "console stopped"
}
