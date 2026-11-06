MOD_DESC="Scaffold a Compose + Roborazzi app from templates/ (adx app-template new <Name> <package>)"

mod_status() {
  local t
  for t in "$ADX_ROOT"/templates/*/; do
    t="$(basename "$t")"; [ "$t" = claude ] && continue
    echo "template: $t"
  done
  [ -d "$ADX_ROOT/templates/compose-roborazzi" ]
}

mod_install() { mod_status; }

# adx app-template new <AppName> <com.example.app> [dest-dir] [template]
mod_new() {
  local name="${1:?usage: adx app-template new <AppName> <package> [dest-dir] [template]}"
  local pkg="${2:?package name required, e.g. com.example.hello}"
  local dest="${3:-$ADX_APPS_DIR/$name}" tpl="${4:-compose-roborazzi}"
  [[ "$name" =~ ^[A-Za-z][A-Za-z0-9]*$ ]] || adx_die "AppName must be alphanumeric, starting with a letter"
  [[ "$pkg" =~ ^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$ ]] || adx_die "invalid package '$pkg'"
  [ -d "$ADX_ROOT/templates/$tpl" ] || adx_die "no template '$tpl'"
  [ -e "$dest" ] && adx_die "$dest already exists"

  export APP_NAME="$name" APP_PACKAGE="$pkg" APP_PACKAGE_PATH="${pkg//.//}"
  adx_log "rendering $tpl -> $dest"
  adx_render_tree "$ADX_ROOT/templates/$tpl" "$dest"
  printf 'sdk.dir=%s\n' "$ANDROID_HOME" > "$dest/local.properties"

  adx_run_module claude-permissions apply "$dest"
  if [ "${ADX_APP_SKILLS:-true}" = true ]; then
    adx_run_module skills apply "$dest"
  fi

  if adx_run_module gradle status >/dev/null 2>&1; then
    # Generate the wrapper in an empty dir so the new project isn't configured yet.
    local w; w="$(mktemp -d)"
    : > "$w/settings.gradle.kts"
    ( cd "$w" && adx_run_module gradle run wrapper --gradle-version "$ADX_GRADLE_VERSION" --distribution-type bin --gradle-distribution-sha256-sum "$ADX_GRADLE_SHA256" -q )
    cp -r "$w/gradlew" "$w/gradlew.bat" "$w/gradle" "$dest/"
    rm -rf "$w"
  else
    adx_warn "gradle module not installed; run 'adx gradle install' then 'adx gradle run wrapper' in $dest"
  fi

  if [ "$ADX_APP_GIT_INIT" = true ] && adx_have git; then
    git -C "$dest" init -q
  fi
  adx_log "done. Next: cd $dest && ./gradlew recordRoborazziDebug"
}
