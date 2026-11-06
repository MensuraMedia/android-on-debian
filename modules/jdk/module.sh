MOD_DESC="Full JDK for Gradle/AGP (user-level Eclipse Temurin, no sudo)"

mod_status() {
  if [ -x "$ADX_JAVA_HOME/bin/javac" ]; then
    echo "jdk: $("$ADX_JAVA_HOME/bin/java" -version 2>&1 | head -1)"
    echo "JAVA_HOME (adx): $ADX_JAVA_HOME"
  else
    echo "no full JDK (javac) at $ADX_JAVA_HOME"; return 1
  fi
}

# Download the latest Temurin build of ADX_JDK_MAJOR from the Adoptium API,
# verify its SHA-256 and unpack it to ADX_JAVA_HOME.
mod_install() {
  local api meta url sha tgz tmp
  api="https://api.adoptium.net/v3/assets/latest/$ADX_JDK_MAJOR/hotspot?architecture=x64&image_type=jdk&os=linux&vendor=eclipse"
  meta="$(curl -fsSL "$api")"
  url="$(jq -r '.[0].binary.package.link' <<<"$meta")"
  sha="$(jq -r '.[0].binary.package.checksum' <<<"$meta")"
  tgz="$ADX_DOWNLOADS/$(basename "$url")"
  adx_log "downloading $(jq -r '.[0].release_name' <<<"$meta")"
  adx_download "$url" "$tgz"
  adx_sha256_check "$tgz" "$sha"
  tmp="$(mktemp -d)"
  tar -xzf "$tgz" -C "$tmp"
  rm -rf "$ADX_JAVA_HOME"
  mkdir -p "$(dirname "$ADX_JAVA_HOME")"
  mv "$tmp"/jdk-* "$ADX_JAVA_HOME"
  rm -rf "$tmp"
  adx_log "installed $ADX_JAVA_HOME"
}
