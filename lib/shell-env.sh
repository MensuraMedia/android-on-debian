# Sourced from ~/.bashrc by the shell-env module. Must be safe in an
# interactive shell: no `set -e`, no output, idempotent.

_adx_env_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$_adx_env_root/config/defaults.env" ]; then
  _adx_old_opts="$-"
  set -a
  . "$_adx_env_root/config/defaults.env"
  [ -f "$_adx_env_root/config/local.env" ] && . "$_adx_env_root/config/local.env"
  case "$_adx_old_opts" in *a*) ;; *) set +a ;; esac
  export ADX_ROOT="$_adx_env_root"
  export ANDROID_HOME
  [ -d "$ADX_JAVA_HOME" ] && export JAVA_HOME="$ADX_JAVA_HOME"
  for _adx_p in "$ADX_ROOT" "$ANDROID_HOME/cmdline-tools/latest/bin" \
                "$ANDROID_HOME/platform-tools" "$ANDROID_HOME/emulator"; do
    case ":$PATH:" in *":$_adx_p:"*) ;; *) PATH="$PATH:$_adx_p" ;; esac
  done
  export PATH
fi
unset _adx_env_root _adx_old_opts _adx_p
