# adx check: the single quality gate. Same script locally, in the pre-commit hook
# and in CI. Sourced by `adx check` only (keeps the core small).
#
#   adx check            changes vs the base (ADX_CHECK_BASE, else merge-base with
#                        origin/main, else HEAD~1, else the empty tree); working tree
#   adx check --staged   exactly what is staged (pre-commit hook): file checks run on
#                        a snapshot of the index, not the working tree; offline
#   adx check --full     also renders the app template and runs its Gradle tests
#
# Each check prints `ok` / `WARN` / `FAIL`; any FAIL exits 1 after ALL sections ran.
# Secrets are never printed: findings name the file and the kind of secret only.

_chk_fail=0 _chk_warn=0
_ok()   { printf '  \033[32mok\033[0m    %s\n' "$*"; }
_warn() { printf '  \033[33mWARN\033[0m  %s\n' "$*"; _chk_warn=$((_chk_warn + 1)); adx_logfile WARN "check: $*"; }
_fail() { printf '  \033[31mFAIL\033[0m  %s\n' "$*"; _chk_fail=$((_chk_fail + 1)); adx_logfile ERROR "check: $*"; }
_section() { printf '\n== %s ==\n' "$*"; }

# Paths whose changes count as "code" (need a CHANGELOG entry, make HANDOFF stale).
_CHK_CODE_RE='^(adx|lib/|modules/|templates/|tools/|config/|profiles/|\.github/|\.claude/workflows/|CLAUDE\.md)'
_CHK_CODE_PATHS=(adx lib modules templates tools config profiles .github .claude/workflows CLAUDE.md)

_git() { git -C "$ADX_ROOT" "$@"; }

# Files changed in scope (relative paths), one per line.
_chk_changed() {
  if [ "$_chk_mode" = staged ]; then
    _git diff --cached --name-only --diff-filter=ACMR
  else
    _git diff --name-only --diff-filter=ACMR "$_chk_base" --
    _git ls-files --others --exclude-standard
  fi | sort -u
}

# Unified diff (no context) of the scope; new untracked files appear as fully added.
_chk_diff() {
  if [ "$_chk_mode" = staged ]; then _git diff --cached -U0
  else
    _git diff -U0 "$_chk_base" --
    local f
    while IFS= read -r f; do
      if [ -f "$ADX_ROOT/$f" ] && grep -Iq . "$ADX_ROOT/$f"; then
        printf '+++ b/%s\n' "$f"; sed 's/^/+/' "$ADX_ROOT/$f"
      fi
    done < <(_git ls-files --others --exclude-standard)
  fi
}

# Tracked files matching a pathspec (from the index when staged), and git grep likewise.
_chk_ls() { if [ "$_chk_mode" = staged ]; then _git ls-files --cached -- "$@"; else _git ls-files --cached --others --exclude-standard -- "$@"; fi; }
_chk_grep() { if [ "$_chk_mode" = staged ]; then _git grep --cached "$@"; else _git grep "$@"; fi; }

_chk_shell() {
  _section shell
  local f bad=0 files=()
  mapfile -t files < <(cd "$_chk_src" && compgen -G adx; compgen -G 'lib/*.sh'; compgen -G 'modules/*/module.sh')
  for f in "${files[@]}"; do bash -n "$_chk_src/$f" || { _fail "bash -n $f"; bad=1; }; done
  if [ "$bad" = 0 ]; then _ok "bash -n (${#files[@]} files)"; fi
  local sc="$HOME/.local/opt/shellcheck-$ADX_SHELLCHECK_VERSION/shellcheck"
  if [ -x "$sc" ]; then
    if (cd "$_chk_src" && "$sc" -x -s bash -S warning "${files[@]}"); then _ok "shellcheck $ADX_SHELLCHECK_VERSION (pinned)"
    else _fail "shellcheck (see above)"; fi
  else
    _fail "shellcheck $ADX_SHELLCHECK_VERSION not installed (adx devtools install)"
  fi
}

_chk_rust() {
  local ws="$_chk_src/tools/aodm" cargo="$ADX_CARGO_HOME/bin/cargo"
  [ -f "$ws/Cargo.toml" ] || return 0
  _section rust
  [ -x "$cargo" ] || { _fail "cargo not found (adx rust install)"; return 0; }
  local tc; tc="$(sed -nE 's/^channel = "([^"]+)"/\1/p' "$ws/rust-toolchain.toml")"
  if [ "$tc" = "$ADX_RUST_TOOLCHAIN" ]; then _ok "toolchain pins agree ($tc)"
  else _fail "rust-toolchain.toml ($tc) != ADX_RUST_TOOLCHAIN ($ADX_RUST_TOOLCHAIN)"; fi
  # A staged snapshot gets its own target dir: sharing the working tree's let cargo
  # treat the snapshot's test binary as the tree's (same relative paths), so the
  # next working-tree run tested a deleted /tmp snapshot. Both caches persist.
  local target="$ADX_ROOT/tools/aodm/target"
  [ "$_chk_mode" = staged ] && target="$target/staged"
  local c=(env CARGO_HOME="$ADX_CARGO_HOME" CARGO_TARGET_DIR="$target" "$cargo")
  if (cd "$ws" && "${c[@]}" fmt --all --check >/dev/null 2>&1); then _ok "cargo fmt"; else _fail "cargo fmt --check (run: cargo fmt --all)"; fi
  if (cd "$ws" && "${c[@]}" clippy --all-targets --locked -q -- -D warnings); then _ok "cargo clippy -D warnings"; else _fail "cargo clippy"; fi
  local out
  if out="$(cd "$ws" && "${c[@]}" test --locked -q 2>&1)"; then
    _ok "cargo test ($(grep -oE '^test result: ok\. [0-9]+ passed' <<<"$out" | awk '{s+=$4} END{print s+0}') passed)"
  else
    _fail "cargo test"; grep -E 'FAILED|panicked' <<<"$out" | head -5
  fi
  local deny="$HOME/.local/opt/cargo-deny-$ADX_CARGO_DENY_VERSION/cargo-deny"
  if [ ! -x "$deny" ]; then _fail "cargo-deny $ADX_CARGO_DENY_VERSION not installed (adx devtools install)"; return 0; fi
  # Pre-commit stays offline: advisories need the network and run in CI.
  local which=(check) label="advisories, licences, sources, bans"
  if [ "$_chk_mode" = staged ]; then which=(--offline check bans licenses sources); label="licences, sources, bans; advisories in CI"; fi
  if (cd "$ws" && PATH="$ADX_CARGO_HOME/bin:$PATH" "$deny" --locked "${which[@]}" -s >/dev/null 2>&1); then _ok "cargo deny ($label)"
  else _fail "cargo deny (run: cd tools/aodm && cargo-deny check)"; fi
}

_chk_docs() {
  _section docs
  local md link target bad=0
  # Relative links resolve. shortcut: links inside code spans/fences are checked too; fine while docs keep fake links out of code.
  while IFS= read -r md; do
    while IFS= read -r link; do
      target="${link%% \"*}"; target="${target%%#*}"; target="${target//%20/ }"
      [ -z "$target" ] && continue
      case "$target" in /*) target="$_chk_src$target" ;; *) target="$(dirname "$_chk_src/$md")/$target" ;; esac
      [ -e "$target" ] || { _fail "broken link in $md: $link"; bad=1; }
    done < <(grep -oE '\]\([^)]+\)' "$_chk_src/$md" | sed -E 's/^\]\(//; s/\)$//' | grep -vE '^(https?|mailto):|^#' || true)
  done < <(_chk_ls '*.md' | grep -vE '^(docs/research/|config/skill-profiles/|\.claude/|tools/aodm/target/)' || true)
  if [ "$bad" = 0 ]; then _ok "relative links"; fi
  # Every `adx <module> [<action>]` in docs exists (module dir + mod_<action>).
  local known="list doctor up env logs check help" planned="release upgrade new-module review" pending="" cmd mod act
  bad=0
  while IFS= read -r cmd; do
    mod="${cmd%% *}"; act=""; [ "$cmd" != "$mod" ] && act="${cmd#* }"
    case " $known " in *" $mod "*) continue ;; esac
    case " $planned " in *" $mod "*) case " $pending " in *" $mod "*) ;; *) pending="$pending $mod" ;; esac; continue ;; esac
    if [ ! -f "$_chk_src/modules/$mod/module.sh" ]; then _fail "docs mention unknown command: adx $mod"; bad=1; continue; fi
    case "$act" in ""|status|install|help) continue ;; esac
    grep -qE "^mod_$act\(\)" "$_chk_src/modules/$mod/module.sh" || { _fail "docs mention unknown action: adx $mod $act"; bad=1; }
  done < <(_chk_grep -hoE '(`|^[[:space:]]*|[$] )(\./)?adx [a-z][a-z-]+( [a-z][a-z-]+)?' -- '*.md' ':!docs/research' \
            | sed -E 's#^.*adx ##' | sort -u)
  if [ "$bad" = 0 ]; then _ok "documented adx commands and actions exist"; fi
  [ -z "$pending" ] || _warn "docs mention planned, not yet built commands:$pending (ROADMAP P9)"
  # Stale terms (with the files allowed to mention them as history).
  local term hits
  bad=0
  for term in 'swiftshader_indirect' 'Downloads/adx/emulator-'; do
    hits="$(_chk_grep -lF "$term" -- '*.md' '*.sh' '*.env' ':!docs/research' ':!lib/check.sh' \
            | grep -vE '(^|/)(CHANGELOG|install-issues|troubleshooting|environment)\.md$|^config/defaults\.env$' || true)"
    [ -z "$hits" ] || { _fail "stale term '$term' in: $(paste -sd ' ' <<<"$hits")"; bad=1; }
  done
  if [ "$bad" = 0 ]; then _ok "no stale terms"; fi
}

_chk_process() {
  _section process
  local changed code
  changed="$(_chk_changed)"
  code="$(grep -E "$_CHK_CODE_RE" <<<"$changed" | grep -v 'Cargo.lock' || true)"
  if [ -z "$code" ]; then _ok "no code changes in scope"; return 0; fi
  # Needs an added line inside [Unreleased] (before the first released "## [x.y.z]" heading).
  local released added
  released="$(grep -nE '^## \[[0-9]' "$_chk_src/CHANGELOG.md" | head -1 | cut -d: -f1)"
  if [ "$_chk_mode" = staged ]; then added="$(_git diff --cached -U0 -- CHANGELOG.md)"
  else added="$(_git diff -U0 "$_chk_base" -- CHANGELOG.md)"; fi
  if awk -v lim="${released:-999999}" '/^@@/ { split($3, a, ","); n = substr(a[1], 2) + 0; c = (a[2] == "" ? 1 : a[2] + 0); if (c > 0 && n < lim) found = 1 } END { exit !found }' <<<"$added"; then
    _ok "CHANGELOG.md [Unreleased] updated with the code change"
  else
    _fail "code changed without an entry under CHANGELOG.md [Unreleased]"
  fi
  local last_code last_handoff
  last_code="$(_git log -1 --format=%ct -- "${_CHK_CODE_PATHS[@]}" 2>/dev/null || echo 0)"
  last_handoff="$(_git log -1 --format=%ct -- HANDOFF.md 2>/dev/null || echo 0)"
  if grep -qx 'HANDOFF.md' <<<"$changed" || [ "${last_handoff:-0}" -ge "${last_code:-0}" ]; then _ok "HANDOFF.md current"
  else _warn "HANDOFF.md older than the last code commit"; fi
}

_chk_security() {
  _section security
  # Secrets: report file + kind only, never content (output also goes to adx.log).
  local hits
  hits="$(_chk_diff | awk '
    /^\+\+\+ / { file = substr($2, 3); next }
    /^\+/ {
      kind = ""
      if ($0 ~ /AKIA[0-9A-Z]{16}/) kind = "AWS access key"
      else if ($0 ~ /-----BEGIN [A-Z ]*PRIVATE KEY/) kind = "private key"
      else if ($0 ~ /gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}/) kind = "GitHub token"
      else if ($0 ~ /xox[baprs]-[A-Za-z0-9-]{10,}/) kind = "Slack token"
      else if ($0 ~ /sk-ant-[A-Za-z0-9_-]{20,}/) kind = "Anthropic API key"
      else if ($0 ~ /sk-(proj|svcacct)-[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9]{32,}/) kind = "OpenAI API key"
      else if ($0 ~ /AIza[0-9A-Za-z_-]{35}/) kind = "Google API key"
      else if ($0 ~ /(store|key)Password[[:space:]]*[=:][[:space:]]*[^[:space:]$]/) kind = "keystore password"
      if (kind != "") print file " (" kind ")"
    }' | sort -u)"
  if [ -z "$hits" ]; then _ok "no secrets in the diff"; else _fail "possible secret(s), content not shown: $(paste -sd ';' <<<"$hits")"; fi
  hits="$(_chk_changed | grep -E '\.(jks|keystore|p12|pem)$|(^|/)local\.properties$|(^|/)google-services\.json$|(^|/)\.env$' || true)"
  if [ -z "$hits" ]; then _ok "no keystores, local.properties or .env files"; else _fail "must not be committed: $(paste -sd ' ' <<<"$hits")"; fi
  # Pipe-to-shell and process-substitution installs (checksum pipes like `| sha256sum` are fine).
  hits="$(cd "$_chk_src" && grep -rnE '(curl|wget)[^#|]*\|([^#|]*\|)*[[:space:]]*(sudo([[:space:]]+-[A-Za-z]+)*[[:space:]]+)?(ba|z|da)?sh([[:space:]]|$)|<\((curl|wget)|(ba|z)?sh[[:space:]]+-c[[:space:]]+"?\$\((curl|wget)|(curl|wget)[^#|]*\|[[:space:]]*python' \
           adx lib modules templates .github 2>/dev/null | grep -v '^lib/check.sh:' || true)"
  if [ -z "$hits" ]; then _ok "no pipe-to-shell installs"; else _fail "pipe-to-shell: $(head -1 <<<"$hits" | cut -c1-120)"; fi
  # Every download is verified: as many adx_sha256_check as adx_download calls, and no raw downloads.
  local m d s bad=0
  for m in "$_chk_src"/modules/*/module.sh; do
    d="$(grep -cE '^[^#]*adx_download ' "$m" || true)"; s="$(grep -cE '^[^#]*adx_sha256_check ' "$m" || true)"
    if [ "$s" -lt "$d" ]; then _fail "$(basename "$(dirname "$m")"): $d download(s), $s checksum check(s)"; bad=1; fi
    if grep -qE '^[^#]*(wget[^|]*[[:space:]]-O[[:space:]]|curl[^|]*[[:space:]]-[a-zA-Z]*[oO][[:space:]])' "$m"; then
      _fail "$(basename "$(dirname "$m")"): raw curl/wget download (use adx_download + adx_sha256_check)"; bad=1
    fi
  done
  if [ "$bad" = 0 ]; then _ok "every module download is checksum-verified"; fi
  hits="$(_chk_changed | grep -E '^(templates/claude/settings\.json|config/claude-global-rules\.md|templates/claude/agents/|\.claude/settings)' || true)"
  if [ -z "$hits" ]; then _ok "no permission/rules/agent files changed"; else _warn "owner approval needed for: $(paste -sd ' ' <<<"$hits")"; fi
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
    rm -rf "$tmp"
  else
    _fail "template app check (rerun manually in $tmp/CheckApp)"
  fi
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
  _chk_src="$ADX_ROOT" _chk_base="" _chk_snap=""
  if [ "$_chk_mode" = staged ]; then
    # Check exactly what will be committed: a snapshot of the index.
    _chk_snap="$(mktemp -d)"
    trap 'rm -rf "$_chk_snap"; adx_logfile END "exit $?"' EXIT
    _git checkout-index -a --prefix="$_chk_snap/"
    _chk_src="$_chk_snap"
  else
    _chk_base="${ADX_CHECK_BASE:-}"
    case "$_chk_base" in 0000000000000000000000000000000000000000) _chk_base="" ;; esac
    if [ -n "$_chk_base" ] && ! _git rev-parse -q --verify "$_chk_base^{commit}" >/dev/null; then _chk_base=""; fi
    if [ -z "$_chk_base" ]; then
      _chk_base="$(_git merge-base HEAD origin/main 2>/dev/null || _git rev-parse -q --verify HEAD~1 || _git hash-object -t tree /dev/null)"
    fi
  fi
  echo "adx check ($_chk_mode$([ "$full" = true ] && echo ", full")${_chk_base:+; base ${_chk_base:0:7}})"
  # Every section runs even when an earlier one fails.
  _chk_shell || true
  _chk_rust || true
  _chk_docs || true
  _chk_process || true
  _chk_security || true
  if [ "$full" = true ]; then _chk_full || true; fi
  printf '\n%s failures, %s warnings, %ss\n' "$_chk_fail" "$_chk_warn" "$((SECONDS - t0))"
  adx_logfile INFO "check $_chk_mode: $_chk_fail failures, $_chk_warn warnings, $((SECONDS - t0))s"
  [ "$_chk_fail" = 0 ]
}
