# adx check: the single quality gate. Same script locally, in the pre-commit hook
# and in CI. Sourced by `adx check` only (keeps the core small).
#
#   adx check            changes vs the main branch (working tree included)
#   adx check --staged   staged changes only (pre-commit hook; fast)
#   adx check --full     also renders the app template and runs its Gradle tests
#
# Each check prints `ok` / `WARN` / `FAIL`; any FAIL exits 1. Duration and counts
# go to the framework log.

_chk_fail=0 _chk_warn=0
_ok()   { printf '  \033[32mok\033[0m    %s\n' "$*"; }
_warn() { printf '  \033[33mWARN\033[0m  %s\n' "$*"; _chk_warn=$((_chk_warn + 1)); adx_logfile WARN "check: $*"; }
_fail() { printf '  \033[31mFAIL\033[0m  %s\n' "$*"; _chk_fail=$((_chk_fail + 1)); adx_logfile ERROR "check: $*"; }
_section() { printf '\n== %s ==\n' "$*"; }

# Files changed in scope (relative paths), one per line.
_chk_changed() {
  if [ "$_chk_mode" = staged ]; then
    git -C "$ADX_ROOT" diff --cached --name-only --diff-filter=ACMR
  else
    git -C "$ADX_ROOT" diff --name-only --diff-filter=ACMR "$_chk_base" -- 2>/dev/null
    git -C "$ADX_ROOT" ls-files --others --exclude-standard
  fi | sort -u
}

# Added lines in scope, as a unified diff; new untracked files count as fully added.
_chk_diff() {
  if [ "$_chk_mode" = staged ]; then git -C "$ADX_ROOT" diff --cached -U0
  else
    git -C "$ADX_ROOT" diff -U0 "$_chk_base" -- 2>/dev/null
    local f
    while IFS= read -r f; do
      [ -f "$ADX_ROOT/$f" ] && grep -Iq . "$ADX_ROOT/$f" && sed 's/^/+/' "$ADX_ROOT/$f"
    done < <(git -C "$ADX_ROOT" ls-files --others --exclude-standard)
  fi
}

_chk_shell() {
  _section shell
  local f bad=0 files=("$ADX_ROOT/adx" "$ADX_ROOT"/lib/*.sh "$ADX_ROOT"/modules/*/module.sh)
  for f in "${files[@]}"; do bash -n "$f" || { _fail "bash -n $f"; bad=1; }; done
  [ "$bad" = 0 ] && _ok "bash -n (${#files[@]} files)"
  if adx_have shellcheck; then
    if (cd "$ADX_ROOT" && shellcheck -x -s bash -S warning adx lib/*.sh modules/*/module.sh); then _ok "shellcheck $(shellcheck --version | awk '/^version/{print $2}')"
    else _fail "shellcheck (see above)"; fi
  else
    _fail "shellcheck not installed (adx devtools install)"
  fi
}

_chk_rust() {
  local ws="$ADX_ROOT/tools/aodm" cargo="$ADX_CARGO_HOME/bin/cargo"
  [ -f "$ws/Cargo.toml" ] || return 0
  _section rust
  [ -x "$cargo" ] || { _fail "cargo not found (adx rust install)"; return; }
  ( cd "$ws" && "$cargo" fmt --all --check >/dev/null 2>&1 ) && _ok "cargo fmt" || _fail "cargo fmt --check (run: cargo fmt --all)"
  if ( cd "$ws" && "$cargo" clippy --all-targets --locked -q -- -D warnings ); then _ok "cargo clippy -D warnings"; else _fail "cargo clippy"; fi
  local out
  if out="$(cd "$ws" && "$cargo" test --locked -q 2>&1)"; then
    _ok "cargo test ($(grep -oE '^test result: ok\. [0-9]+ passed' <<<"$out" | awk '{s+=$4} END{print s+0}') passed)"
  else
    _fail "cargo test"; grep -E 'FAILED|panicked' <<<"$out" | head -5
  fi
  if adx_have cargo-deny; then
    ( cd "$ws" && PATH="$ADX_CARGO_HOME/bin:$PATH" cargo-deny --locked check -s >/dev/null 2>&1 ) && _ok "cargo deny (advisories, licences, sources, bans)" || _fail "cargo deny (run: cd tools/aodm && cargo deny check)"
  else
    _fail "cargo-deny not installed (adx devtools install)"
  fi
}

_chk_docs() {
  _section docs
  local md link target bad=0 term
  # Relative links resolve (research notes and vendored skills excluded).
  while IFS= read -r md; do
    while IFS= read -r link; do
      target="${link%%#*}"; target="${target//%20/ }"
      [ -z "$target" ] && continue
      [ -e "$(dirname "$md")/$target" ] || { _fail "broken link in ${md#"$ADX_ROOT"/}: $link"; bad=1; }
    done < <(grep -oE '\]\([^)#:]+(#[^)]*)?\)' "$md" | sed -E 's/^\]\(//; s/\)$//' | grep -vE '^(https?|mailto):')
  done < <(find "$ADX_ROOT" -name '*.md' -not -path '*/.git/*' -not -path '*/.claude/*' -not -path '*/docs/research/*' -not -path '*/target/*' -not -path '*/config/skill-profiles/*')
  [ "$bad" = 0 ] && _ok "relative links"
  # Every `adx <word>` in code (backticks or command lines) is a core command or a module.
  local cmd known="list doctor up env logs check help" planned="release upgrade new-module" pending=""
  bad=0
  while IFS= read -r cmd; do
    case " $known " in *" $cmd "*) continue ;; esac
    case " $planned " in *" $cmd "*) pending="$pending $cmd"; continue ;; esac
    [ -d "$ADX_ROOT/modules/$cmd" ] || { _fail "docs mention unknown command: adx $cmd"; bad=1; }
  done < <(git -C "$ADX_ROOT" grep -hoE '(`|^[[:space:]]*|[$] )(\./)?adx [a-z][a-z-]+' -- '*.md' ':!docs/research' \
            | sed -E 's#^.*adx ##' | sort -u)
  [ "$bad" = 0 ] && _ok "documented adx commands exist"
  [ -z "$pending" ] || _warn "docs mention planned, not yet built commands:$pending (ROADMAP P9)"
  # Stale terms (with the files allowed to mention them as history).
  bad=0
  for term in 'swiftshader_indirect' 'Downloads/adx/emulator-'; do
    local hits
    hits="$(git -C "$ADX_ROOT" grep -lF "$term" -- '*.md' '*.sh' '*.env' ':!docs/research' ':!lib/check.sh' \
            | grep -vE '(^|/)(CHANGELOG|install-issues|troubleshooting|environment)\.md$|^config/defaults\.env$' || true)"
    [ -z "$hits" ] || { _fail "stale term '$term' in: $(paste -sd ' ' <<<"$hits")"; bad=1; }
  done
  [ "$bad" = 0 ] && _ok "no stale terms"
}

_chk_process() {
  _section process
  local changed code
  changed="$(_chk_changed)"
  code="$(grep -E '^(adx|lib/|modules/|templates/|tools/|config/|profiles/|\.github/)' <<<"$changed" | grep -v 'Cargo.lock' || true)"
  if [ -z "$code" ]; then _ok "no code changes in scope"; return; fi
  if grep -qx 'CHANGELOG.md' <<<"$changed"; then _ok "CHANGELOG.md updated with the code change"
  else _fail "code changed without a CHANGELOG.md [Unreleased] entry"; fi
  local last_code last_handoff
  last_code="$(git -C "$ADX_ROOT" log -1 --format=%ct -- adx lib modules templates tools config profiles 2>/dev/null || echo 0)"
  last_handoff="$(git -C "$ADX_ROOT" log -1 --format=%ct -- HANDOFF.md 2>/dev/null || echo 0)"
  if grep -qx 'HANDOFF.md' <<<"$changed" || [ "${last_handoff:-0}" -ge "${last_code:-0}" ]; then _ok "HANDOFF.md current"
  else _warn "HANDOFF.md older than the last code commit"; fi
}

_chk_security() {
  _section security
  local diff hits
  diff="$(_chk_diff | grep -E '^\+' | grep -vE '^\+\+\+' || true)"
  hits="$(grep -nE 'AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|xox[baprs]-[A-Za-z0-9-]{10,}|sk-[A-Za-z0-9]{32,}' <<<"$diff" || true)"
  # Never echo the secret itself (failures also go to adx.log): keep 4 characters.
  [ -z "$hits" ] && _ok "no secrets in the diff" \
    || _fail "possible secret in the diff ($(wc -l <<<"$hits") line(s)), e.g. $(head -1 <<<"$hits" | sed -E 's/(AKIA|gh[pousr]_|github_pat_|xox[baprs]-|sk-|-----BEGIN)[A-Za-z0-9_ -]{4,}/\1****/g' | cut -c1-60)"
  hits="$(grep -rnE '(curl|wget)[^|]*\|[[:space:]]*(sudo[[:space:]]+)?(ba|z)?sh' "$ADX_ROOT/adx" "$ADX_ROOT/lib" "$ADX_ROOT/modules" | grep -v 'lib/check.sh' || true)"
  [ -z "$hits" ] && _ok "no curl|sh" || _fail "pipe-to-shell: $(head -1 <<<"$hits")"
  local m bad=0
  for m in "$ADX_ROOT"/modules/*/module.sh; do
    if grep -q 'adx_download' "$m" && ! grep -q 'adx_sha256_check' "$m"; then _fail "$(basename "$(dirname "$m")"): downloads without adx_sha256_check"; bad=1; fi
  done
  [ "$bad" = 0 ] && _ok "every module download is checksum-verified"
  hits="$(_chk_changed | grep -E '^(templates/claude/settings\.json|config/claude-global-rules\.md|templates/claude/agents/)' || true)"
  [ -z "$hits" ] && _ok "no permission/rules/agent files changed" || _warn "owner approval needed for: $(paste -sd ' ' <<<"$hits")"
}

_chk_full() {
  _section "app template (--full)"
  local tmp; tmp="$(mktemp -d)"
  if ADX_APP_SKILLS=false ADX_APP_GIT_INIT=false "$ADX_ROOT/adx" app-template new CheckApp com.example.checkapp "$tmp/CheckApp" >/dev/null 2>&1 \
     && ! grep -rqE '@@[A-Z0-9_]+@@' "$tmp/CheckApp" --exclude-dir=.gradle \
     && (cd "$tmp/CheckApp" && ./gradlew -q testDebugUnitTest >/dev/null 2>&1 \
         && ./gradlew -q recordRoborazziDebug >/dev/null 2>&1 \
         && ./gradlew -q verifyRoborazziDebug >/dev/null 2>&1); then
    # A new app has no baselines yet: record, then verify proves the renders are deterministic.
    _ok "template renders, no unrendered tokens, tests pass, screenshots record + verify (deterministic)"
  else
    _fail "template app check (rerun manually in $tmp/CheckApp)"; return
  fi
  rm -rf "$tmp"
}

adx_cmd_check() {
  _chk_mode=branch
  local full=false a t0=$SECONDS
  for a in "$@"; do
    case "$a" in
      --staged) _chk_mode=staged ;;
      --full)   full=true ;;
      *) adx_die "usage: adx check [--staged] [--full]" ;;
    esac
  done
  _chk_base="$(git -C "$ADX_ROOT" merge-base HEAD origin/main 2>/dev/null || git -C "$ADX_ROOT" rev-parse HEAD~1)"
  echo "adx check ($_chk_mode$([ "$full" = true ] && echo ", full"); base ${_chk_base:0:7})"
  _chk_shell
  _chk_rust
  _chk_docs
  _chk_process
  _chk_security
  if [ "$full" = true ]; then _chk_full; fi
  printf '\n%s failures, %s warnings, %ss\n' "$_chk_fail" "$_chk_warn" "$((SECONDS - t0))"
  adx_logfile INFO "check $_chk_mode: $_chk_fail failures, $_chk_warn warnings, $((SECONDS - t0))s"
  [ "$_chk_fail" = 0 ]
}
