MOD_DESC="Bootstrap Gradle distribution (only used to generate wrappers in new apps)"

_gradle_home() { echo "$HOME/.local/opt/gradle/gradle-$ADX_GRADLE_VERSION"; }

mod_status() {
  if [ -x "$(_gradle_home)/bin/gradle" ]; then
    echo "gradle $ADX_GRADLE_VERSION at $(_gradle_home)"
  else
    echo "gradle $ADX_GRADLE_VERSION not installed"; return 1
  fi
}

mod_install() {
  local zip="$ADX_DOWNLOADS/gradle-$ADX_GRADLE_VERSION-bin.zip"
  adx_log "downloading gradle $ADX_GRADLE_VERSION"
  adx_download "https://services.gradle.org/distributions/gradle-$ADX_GRADLE_VERSION-bin.zip" "$zip"
  adx_sha256_check "$zip" "$ADX_GRADLE_SHA256"
  mkdir -p "$HOME/.local/opt/gradle"
  unzip -q -o "$zip" -d "$HOME/.local/opt/gradle"
  adx_log "installed $(_gradle_home)"
}

# Run the bootstrap gradle: adx gradle run wrapper --gradle-version X
mod_run() { JAVA_HOME="$ADX_JAVA_HOME" "$(_gradle_home)/bin/gradle" "$@"; }
