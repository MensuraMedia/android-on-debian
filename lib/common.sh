# Shared helpers for adx and its modules. Sourced, never executed.

adx_log()  { printf '\033[1;34m[adx]\033[0m %s\n' "$*"; }
adx_warn() { printf '\033[1;33m[adx] warn:\033[0m %s\n' "$*" >&2; }
adx_die()  { printf '\033[1;31m[adx] error:\033[0m %s\n' "$*" >&2; exit 1; }
adx_have() { command -v "$1" >/dev/null 2>&1; }

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
  [ -z "${2:-}" ] && return 0
  echo "$2  $1" | sha256sum -c --quiet - || adx_die "checksum mismatch for $1"
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
  sed -i "/^# >>> adx:$name >>>\$/,/^# <<< adx:$name <<<\$/d" "$file"
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
