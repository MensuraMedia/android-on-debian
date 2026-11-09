# Shared helpers for adx and its modules. Sourced, never executed.

# Every message is shown on the terminal and appended, timestamped and without
# colour, to the framework log: $ADX_LOG_DIR/adx.log (see docs/environment.md).
adx_log()  { printf '\033[1;34m[adx]\033[0m %s\n' "$*"; adx_logfile INFO "$*"; }
adx_warn() { printf '\033[1;33m[adx] warn:\033[0m %s\n' "$*" >&2; adx_logfile WARN "$*"; }
adx_die()  { printf '\033[1;31m[adx] error:\033[0m %s\n' "$*" >&2; adx_logfile ERROR "$*"; exit 1; }
adx_have() { command -v "$1" >/dev/null 2>&1; }

# Append one line to the framework log: adx_logfile <LEVEL> <message...>
# Never fails the caller (logging must not break a command).
adx_logfile() {
  local level="$1"; shift
  [ -n "${ADX_LOG_DIR:-}" ] || return 0
  {
    mkdir -p "$ADX_LOG_DIR" &&
    printf '%s %-5s [%s%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$level" \
      "${MOD_NAME:-adx}" "${ADX_RUN_ID:+ #$ADX_RUN_ID}" "$(printf '%s' "$*" | tr '\n\r' '||')" >> "$ADX_LOG_DIR/adx.log"
  } 2>/dev/null || true
}

# Keep a log file below ADX_LOG_MAX_KB by shifting it to <file>.1 ... <file>.N
#   adx_log_rotate <file> [keep=ADX_LOG_KEEP]
adx_log_rotate() {
  local f="$1" keep="${2:-${ADX_LOG_KEEP:-5}}" i
  [ -f "$f" ] || return 0
  [ "$(stat -c %s "$f")" -ge "$(( ${ADX_LOG_MAX_KB:-5120} * 1024 ))" ] || return 0
  for ((i = keep - 1; i >= 1; i--)); do
    [ -f "$f.$i" ] && mv -f "$f.$i" "$f.$((i + 1))"
  done
  mv -f "$f" "$f.1"
}

# Keep the previous run's log of a long-running process (e.g. the emulator),
# so a crash log survives the next start: <file> -> <file>.1 ... <file>.N
adx_log_shift() {
  local f="$1" keep="${2:-${ADX_LOG_KEEP:-5}}" i
  [ -f "$f" ] || return 0
  for ((i = keep - 1; i >= 1; i--)); do
    [ -f "$f.$i" ] && mv -f "$f.$i" "$f.$((i + 1))"
  done
  mv -f "$f" "$f.1"
}

# Load config/defaults.env, then config/local.env (machine overrides, git-ignored).
adx_load_config() {
  set -a
  # shellcheck source=../config/defaults.env
  . "$ADX_ROOT/config/defaults.env"
  [ -f "$ADX_ROOT/config/local.env" ] && . "$ADX_ROOT/config/local.env"
  set +a
}

adx_modules() {
  local d
  for d in "$ADX_ROOT"/modules/*/module.sh; do
    basename "$(dirname "$d")"
  done
}

adx_module_desc() {
  grep -m1 '^MOD_DESC=' "$ADX_ROOT/modules/$1/module.sh" | cut -d= -f2- | tr -d '"'
}

# Run one action of a module in a subshell so modules cannot leak state.
# Contract: modules/<name>/module.sh defines MOD_DESC and functions
# mod_status (0 = ok) and mod_install; any mod_<action> is a public action.
adx_run_module() {
  local name="$1" action="$2"; shift 2
  local file="$ADX_ROOT/modules/$name/module.sh"
  [ -f "$file" ] || adx_die "unknown module '$name' (try: adx list)"
  (
    MOD_NAME="$name"
    MOD_DIR="$ADX_ROOT/modules/$name"
    # shellcheck disable=SC1090
    . "$file"
    if [ "$action" = help ]; then
      echo "$name: $MOD_DESC"
      echo "actions: $(declare -F | awk '{print $3}' | grep '^mod_' | sed 's/^mod_//' | tr '\n' ' ')"
      exit 0
    fi
    declare -F "mod_$action" >/dev/null || adx_die "module '$name' has no action '$action' (try: adx $name help)"
    "mod_$action" "$@"
  )
}

# Resumable download with retries: adx_download <url> <dest-file>
adx_download() {
  mkdir -p "$(dirname "$2")"
  wget -q -c --timeout=30 --tries=10 -O "$2" "$1"
}

# Verify a file's SHA-256 if an expected hash is given (empty = skip).
adx_sha256_check() {
  if [ -z "${2:-}" ]; then
    [ "${ADX_ALLOW_UNVERIFIED:-0}" = 1 ] && { adx_warn "no checksum for $1 (ADX_ALLOW_UNVERIFIED=1)"; return 0; }
    adx_die "no checksum pinned for $1 (set it in config, or ADX_ALLOW_UNVERIFIED=1 to accept the risk)"
  fi
  echo "$2  $1" | sha256sum -c --quiet - || { rm -f "$1"; adx_die "checksum mismatch for $1 (deleted; rerun to download again)"; }
}

# Write or replace a marked block in a file:
#   adx_block_write <file> <block-name> <content>
adx_block_write() {
  local file="$1" name="$2" content="$3"
  local begin="# >>> adx:$name >>>" end="# <<< adx:$name <<<"
  touch "$file"
  adx_block_remove "$file" "$name"
  printf '%s\n%s\n%s\n' "$begin" "$content" "$end" >> "$file"
}

adx_block_remove() {
  local file="$1" name="$2"
  [ -f "$file" ] || return 0
  sed -i --follow-symlinks "/^# >>> adx:$name >>>\$/,/^# <<< adx:$name <<<\$/d" "$file"
}

adx_block_present() {
  grep -q "^# >>> adx:$2 >>>\$" "$1" 2>/dev/null
}

# Copy a template tree, replacing @@VAR@@ in file contents and paths with the
# value of environment variable VAR (unknown tokens are left untouched).
#   adx_render_tree <src-dir> <dest-dir>
adx_render_tree() {
  local src="$1" dst="$2" rel out
  ( cd "$src" && find . -type f ) | while read -r rel; do
    rel="${rel#./}"
    out="$dst/$(printf '%s' "$rel" | perl -pe 's/@@([A-Z0-9_]+)@@/exists $ENV{$1} ? $ENV{$1} : $&/ge')"
    mkdir -p "$(dirname "$out")"
    if grep -Iq . "$src/$rel" 2>/dev/null; then
      perl -pe 's/@@([A-Z0-9_]+)@@/exists $ENV{$1} ? $ENV{$1} : $&/ge' "$src/$rel" > "$out"
    else
      cp "$src/$rel" "$out"
    fi
    if [ -x "$src/$rel" ]; then chmod +x "$out"; fi
  done
}

# Set KEY="value" in config/local.env unless the user already set KEY there.
# Returns 0 if it wrote the value, 1 if KEY was already present.
#   adx_local_set <KEY> <value> <comment>
adx_local_set() {
  local f="$ADX_ROOT/config/local.env"
  grep -qE "^[[:space:]]*(export[[:space:]]+)?$1=" "$f" 2>/dev/null && return 1
  [ -f "$f" ] || printf '# Machine overrides (git-ignored). See config/local.env.example.\n' > "$f"
  printf '\n# %s (set by adx on %s)\n%s=%q\n' "$3" "$(date +%F)" "$1" "$2" >> "$f"
  adx_logfile INFO "local.env: $1=\"$2\""
}
