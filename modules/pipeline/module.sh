MOD_DESC="App pipeline: build, lint, test, deploy and smoke-test an app on the emulator (adx pipeline run <App>)"

_py() { echo "$ADX_ROOT/tools/pipeline/pipeline.py"; }

# Everything the runner needs: JDK and SDK for Gradle and adb, and where runs go.
_pipeline() {
  adx_have python3 || adx_die "python3 not found (install it with your package manager)"
  mkdir -p "$ADX_PIPELINE_DIR" && chmod 700 "$ADX_PIPELINE_DIR"   # app logs and screens: this user only
  JAVA_HOME="$ADX_JAVA_HOME" ANDROID_HOME="$ANDROID_HOME" ADX_APPS_DIR="$ADX_APPS_DIR" \
    ADX_LOG_DIR="$ADX_LOG_DIR" ADX_PIPELINE_DIR="$ADX_PIPELINE_DIR" \
    ADX_PIPELINE_KEEP="$ADX_PIPELINE_KEEP" ADX_PIPELINE_SMOKE_SECS="$ADX_PIPELINE_SMOKE_SECS" \
    ADX_PIPELINE_IMAGE="${ADX_PIPELINE_IMAGE:-$ADX_AVD_NAME}" ADX_PIPELINE_GRADLE_TIMEOUT="$ADX_PIPELINE_GRADLE_TIMEOUT" \
    PATH="$ADX_JAVA_HOME/bin:$HOME/.local/bin:$PATH" \
    python3 "$(_py)" "$@"
}

mod_status() {
  local rc=0
  if adx_have python3; then echo "python3: $(python3 --version 2>&1)"; else echo "python3 missing"; rc=1; fi
  if [ -x "$ADX_JAVA_HOME/bin/java" ]; then echo "jdk: $ADX_JAVA_HOME"; else echo "jdk missing (adx jdk install)"; rc=1; fi
  echo "runs: $ADX_PIPELINE_DIR (last $ADX_PIPELINE_KEEP per app)"
  return "$rc"
}

# Nothing to download: Python standard library, plus the jdk/sdk/emulator modules.
mod_install() { mod_status; }

# adx pipeline run <App|dir> [--serial emulator-NNNN] [--stages build,check,test,deploy,smoke] [--json]
mod_run()  { _pipeline run "$@"; }
# adx pipeline last <App> [--json]   the latest run's summary (or run.json)
mod_last() { _pipeline last "$@"; }
# adx pipeline list <App> [--json]   all kept runs, newest first
mod_list() { _pipeline list "$@"; }
