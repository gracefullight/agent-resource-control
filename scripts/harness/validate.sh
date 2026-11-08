#!/usr/bin/env bash
set -e
set -u
RED='\033[0;31m'
YLW='\033[0;33m'
GRN='\033[0;32m'
BLU='\033[0;34m'
RST='\033[0m'
info()    { printf "${BLU}[validate]${RST} %s\n" "$*"; }
ok()      { printf "${GRN}[ok]${RST}       %s\n" "$*"; }
warn()    { printf "${YLW}[warn]${RST}     %s\n" "$*" >&2; }
violation() {
  printf "${RED}[VIOLATION]${RST} %s\n" "$*" >&2
}
MODE="${1:-}"
if [ "${MODE}" = "--staged" ]; then
  FILES=$(git diff --cached --name-only --diff-filter=ACM 2>/dev/null || true)
else
  FILES=$(git ls-files 2>/dev/null || true)
fi
if [ -z "${FILES}" ]; then
  ok "No files to validate."
  exit 0
fi
info "Mode: ${MODE:-all-tracked}"
info "Files to check: $(echo "${FILES}" | wc -l | tr -d ' ')"
VIOLATIONS=0
SCAN_DIR=$(mktemp -d -t av-validate-scan-XXXXXX)
trap 'rm -rf "${SCAN_DIR}"' EXIT
scan_eligible() {
  local check="$1" path="$2"
  [ -f "$path" ] || return 1
  case "$check" in
    1)
      case "$path" in
        .env.example|scripts/harness/validate.sh|package-lock.json|yarn.lock|go.sum|poetry.lock) return 1 ;;
        *.png|*.jpg|*.jpeg|*.gif|*.ico|*.woff|*.woff2|*.ttf|*.eot) return 1 ;;
      esac ;;
    2)
      case "$path" in
        *.sh|*.bash|*.zsh|.github/workflows/*.yml|.github/workflows/*.yaml) ;;
        *) return 1 ;;
      esac
      [ "$path" != scripts/harness/validate.sh ] || return 1 ;;
    3)
      case "$path" in */domain/*|domain/*) ;; *) return 1 ;; esac ;;
  esac
}
scan_prepare() {
  SCAN_CHECK="$1"
  SCAN_CACHE=()
  BATCH_FILES=()
  SCAN_COUNT=${#ELIGIBLE[@]}
  : > "${SCAN_DIR}/files"
  local i=0
  while [ "$i" -lt "$SCAN_COUNT" ]; do
    printf '%s\t%s\n' "${ELIGIBLE[$i]}" "$i" >> "${SCAN_DIR}/files"
    case "${ELIGIBLE[$i]}" in
      -*) ;;
      *) BATCH_FILES[${#BATCH_FILES[@]}]="${ELIGIBLE[$i]}" ;;
    esac
    i=$((i + 1))
  done
}
scan_frame() {
  local kind="$1" prefix line file body
  if [ "$kind" = trigger ]; then
    while IFS= read -r -d '' file; do printf '%s\t1\n' "$file"; done
  else
    while IFS= read -r -d '' prefix || [ -n "$prefix" ]; do
      while [[ "$prefix" = *$'\n'* ]]; do
        line="${prefix%%$'\n'*}"; prefix="${prefix#*$'\n'}"
        case "$line" in
          'Binary file '*' matches')
            file="${line#Binary file }"; file="${file% matches}"
            printf '%s\t%s\n' "$file" "$line" ;;
        esac
      done
      if [ -n "$prefix" ]; then
        IFS= read -r body || true
        printf '%s\t%s\n' "$prefix" "$body"
      fi
    done
  fi
}
scan_collect() {
  local kind="$1" expression="$2" filter="${3:-}"
  [ "${#BATCH_FILES[@]}" -gt 0 ] || return 0
      if [ "$kind" = trigger ]; then
        printf '%s\0' "${BATCH_FILES[@]}" | xargs -0 grep -lE --null "$expression" 2>/dev/null || true
      else
        printf '%s\0' "${BATCH_FILES[@]}" | xargs -0 grep -nHE --null "$expression" 2>/dev/null || true
      fi | scan_frame "$kind" | awk -v map="${SCAN_DIR}/files" '
        BEGIN {
          while ((getline line < map) > 0) {
            tab = index(line, "\t"); ids[substr(line, 1, tab-1)] = substr(line, tab+1)
          }
          close(map)
        }
        {
          tab = index($0, "\t"); file = substr($0, 1, tab-1)
          if (file in ids) print ids[file] "\t" substr($0, tab+1)
        }' | if [ -n "$filter" ]; then grep -vE "$filter" || true; else cat; fi
}
scan_store() {
  local rule="$1" slot row index body
  while IFS= read -r row; do
    index="${row%%$'\t'*}"
    body="${row#*$'\t'}"
    slot=$((rule * SCAN_COUNT + index))
    SCAN_CACHE[$slot]="${SCAN_CACHE[$slot]-}${body}"$'\n'
  done
}
scan_batch() {
  local rule="$1" kind="$2" expression="$3" filter="${4:-}"
  scan_store "$rule" < <(scan_collect "$kind" "$expression" "$filter")
}
scan_matches() {
  case "$file" in
    -*)
      local expression="$3" filter="${4:-}"
      matches=""
      if [ "${5:-}" = report ] || grep -qE "$expression" "$file" 2>/dev/null; then
        if [ -n "$filter" ]; then
          matches=$(grep -nE "$expression" "$file" 2>/dev/null | grep -vE "$filter" || true)
        else
          matches=$(grep -nE "$expression" "$file" 2>/dev/null || true)
        fi
      fi
      return 0 ;;
  esac
  local slot=$(( $1 * SCAN_COUNT + $2 ))
  matches="${SCAN_CACHE[$slot]-}"
  matches="${matches%$'\n'}"
}
scan_replay_index() {
  scan_eligible "$SCAN_CHECK" "$file" || return 1
  while [ "$file_index" -lt "$SCAN_COUNT" ]; do
    [ "${ELIGIBLE[$file_index]}" != "$file" ] || return 0
    file_index=$((file_index + 1))
  done
  return 1
}
check_pattern() {
  local description="$1"
  local pattern="$2"
  local file_filter="${3:-}"
  while IFS= read -r file; do
    [ -f "${file}" ] || continue
    if [ -n "${file_filter}" ]; then
      case "${file}" in
        ${file_filter}) ;;
        *) continue ;;
      esac
    fi
    local matches
    if grep -qP "${pattern}" "${file}" 2>/dev/null; then
      matches=$(grep -nP "${pattern}" "${file}" 2>/dev/null || true)
    elif grep -qE "${pattern}" "${file}" 2>/dev/null; then
      matches=$(grep -nE "${pattern}" "${file}" 2>/dev/null || true)
    else
      continue
    fi
    if [ -n "${matches}" ]; then
      violation "${description}"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  done <<< "${FILES}"
}
info "Check 1/5: Hardcoded secrets"
EXCLUDE_SECRET_FILES=".env.example|scripts/harness/validate.sh|package-lock.json|yarn.lock|go.sum|poetry.lock"
ELIGIBLE=()
while IFS= read -r file; do
  scan_eligible 1 "$file" || continue
  ELIGIBLE[${#ELIGIBLE[@]}]="$file"
done <<< "${FILES}"
  SECRET_PATTERNS=(
    'lin_api_[A-Za-z0-9]{20,}'
    'API_KEY\s*=\s*["\x27][A-Za-z0-9_\-]{16,}["\x27]'
    'API_SECRET\s*=\s*["\x27][A-Za-z0-9_\-]{16,}["\x27]'
    'ACCESS_TOKEN\s*=\s*["\x27][A-Za-z0-9_\-\.]{16,}["\x27]'
    'PRIVATE_KEY\s*=\s*["\x27][A-Za-z0-9_\-]{16,}["\x27]'
    'AKIA[0-9A-Z]{16}'
    'ghp_[A-Za-z0-9]{36}'
    'github_pat_[A-Za-z0-9_]{82}'
    'Bearer\s+[A-Za-z0-9\._\-]{32,}'
  )
scan_prepare 1
rule=0
SCAN_PIDS=()
for pattern in "${SECRET_PATTERNS[@]}"; do
  scan_collect report "$pattern" > "${SCAN_DIR}/secret-${rule}" &
  SCAN_PIDS[${#SCAN_PIDS[@]}]=$!
  rule=$((rule + 1))
  if [ $((rule % 3)) -eq 0 ]; then
    for scan_pid in "${SCAN_PIDS[@]}"; do wait "$scan_pid"; done
    SCAN_PIDS=()
  fi
done
if [ "${#SCAN_PIDS[@]}" -gt 0 ]; then
  for scan_pid in "${SCAN_PIDS[@]}"; do wait "$scan_pid"; done
fi
rule=0
while [ "$rule" -lt "${#SECRET_PATTERNS[@]}" ]; do
  scan_store "$rule" < "${SCAN_DIR}/secret-${rule}"
  rule=$((rule + 1))
done
file_index=0
while IFS= read -r file; do
  scan_replay_index || continue
  rule=0
  for pattern in "${SECRET_PATTERNS[@]}"; do
    scan_matches "$rule" "$file_index" "$pattern"
    rule=$((rule + 1))
    if [ -n "${matches}" ]; then
      violation "Hardcoded secret detected (pattern: ${pattern})"
      violation "  Fix: Move the value to .env and reference it via environment variable."
      violation "  See: docs/architecture/CONSTRAINTS.md section 3"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  done
  file_index=$((file_index + 1))
done <<< "${FILES}"
[ "${VIOLATIONS}" -eq 0 ] && ok "No hardcoded secrets found" || true
info "Check 2/5: Dangerous shell commands"
VIOLATIONS_BEFORE_2="${VIOLATIONS}"
ELIGIBLE=()
while IFS= read -r file; do
  scan_eligible 2 "$file" || continue
  ELIGIBLE[${#ELIGIBLE[@]}]="$file"
done <<< "${FILES}"
scan_prepare 2
  scan_batch 0 report 'rm\s+-[rf]{1,2}\s+/'
  scan_batch 3 trigger 'git\s+push\s+.*--force.*\s+(main|master)'
  scan_batch 4 trigger 'git\s+push\s+.*-f\s+.*(main|master)'
  scan_batch 1 report 'git\s+push\s+.*(--force|-f).*(main|master)'
  scan_batch 2 report 'git\s+push\s+(--force|-f)\s*$'
file_index=0
while IFS= read -r file; do
  scan_replay_index || continue
  scan_matches 0 "$file_index" 'rm\s+-[rf]{1,2}\s+/'
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Dangerous: rm -rf with absolute path root"
      violation "  Fix: Use a scoped path variable, never start from / or \$HOME directly."
      violation "  See: docs/harness/SAFETY.md section 1"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  trigger_one=""
  case "$file" in
    -*)
      if grep -qE 'git\s+push\s+.*--force.*\s+(main|master)' "$file" 2>/dev/null || \
         grep -qE 'git\s+push\s+.*-f\s+.*(main|master)' "$file" 2>/dev/null; then
        trigger_one=1
      fi ;;
    *)
      scan_matches 3 "$file_index"
      trigger_one="$matches"
      scan_matches 4 "$file_index"
      trigger_one="$trigger_one$matches" ;;
  esac
  if [ -n "$trigger_one" ]; then
    scan_matches 1 "$file_index" 'git\s+push\s+.*(--force|-f).*(main|master)' "" report
    if [ -n "${matches}" ]; then
      violation "Dangerous: git push --force to main/master"
      violation "  Fix: Never force-push to main or master."
      violation "  See: docs/harness/SAFETY.md section 1, AGENTS.md section 3"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  scan_matches 2 "$file_index" 'git\s+push\s+(--force|-f)\s*$'
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Dangerous: bare git push --force (ambiguous target branch)"
      violation "  Fix: Always specify target branch explicitly and confirm it is not main."
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  file_index=$((file_index + 1))
done <<< "${FILES}"
[ "${VIOLATIONS}" -eq "${VIOLATIONS_BEFORE_2}" ] && ok "No dangerous shell commands found" || true
info "Check 3/5: Architecture layer violations (domain must be pure)"
VIOLATIONS_BEFORE_3="${VIOLATIONS}"
NON_DOMAIN_LAYER_DIRS="tracker|workspace|sessions|observability|config|orchestrator|relay|server|infrastructure"
ELIGIBLE=()
while IFS= read -r file; do
  scan_eligible 3 "$file" || continue
  ELIGIBLE[${#ELIGIBLE[@]}]="$file"
done <<< "${FILES}"
scan_prepare 3
  scan_batch 0 report "from ['\"][^'\"]*(/|^)(${NON_DOMAIN_LAYER_DIRS})(/|['\"])" "from ['\"]\./"
  scan_batch 1 report "from ['\"].*infrastructure"
  scan_batch 2 report "^(from|import)\s+.*infrastructure"
  scan_batch 3 report '"[^"]+/infrastructure[^"]*"'
  scan_batch 4 report "^(from|import)\s+(sqlalchemy|django\.db|peewee|tortoise|motor|pymongo|linear_sdk)"
file_index=0
while IFS= read -r file; do
  scan_replay_index || continue
  scan_matches 0 "$file_index" "from ['\"][^'\"]*(/|^)(${NON_DOMAIN_LAYER_DIRS})(/|['\"])" "from ['\"]\./"
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Architecture violation: domain/ imports from non-domain layer"
      violation "  Fix: Domain must be pure. Define an interface in domain/, implement it in the outer layer, and inject it."
      violation "  See: docs/architecture/LAYERS.md, docs/architecture/CONSTRAINTS.md section 1"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  scan_matches 1 "$file_index" "from ['\"].*infrastructure"
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Architecture violation: domain/ imports from infrastructure/"
      violation "  Fix: Domain layer must not depend on infrastructure."
      violation "  Move the dependency inversion — define an interface in domain/, implement it in infrastructure/."
      violation "  See: docs/architecture/CONSTRAINTS.md section 1"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  scan_matches 2 "$file_index" "^(from|import)\s+.*infrastructure"
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Architecture violation: domain/ imports from infrastructure/ (Python)"
      violation "  Fix: Use dependency inversion — define abstract interface in domain/, inject concrete impl."
      violation "  See: docs/architecture/CONSTRAINTS.md section 1, docs/architecture/enforcement/python.md"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  scan_matches 3 "$file_index" '"[^"]+/infrastructure[^"]*"'
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Architecture violation: domain/ imports from infrastructure/ (Go)"
      violation "  Fix: Define interfaces in domain/, inject infrastructure implementations via constructors."
      violation "  See: docs/architecture/CONSTRAINTS.md section 1, docs/architecture/enforcement/go.md"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  scan_matches 4 "$file_index" "^(from|import)\s+(sqlalchemy|django\.db|peewee|tortoise|motor|pymongo|linear_sdk)"
  if [ -n "$matches" ]; then
    if [ -n "${matches}" ]; then
      violation "Architecture violation: ORM or external SDK imported in domain/ (Python)"
      violation "  Fix: Domain layer must be pure. Move persistence logic to infrastructure/."
      violation "  See: docs/architecture/CONSTRAINTS.md section 1"
      while IFS= read -r match_line; do
        violation "  ${file}:${match_line}"
      done <<< "${matches}"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  file_index=$((file_index + 1))
done <<< "${FILES}"
[ "${VIOLATIONS}" -eq "${VIOLATIONS_BEFORE_3}" ] && ok "No architecture layer violations found" || true
info "Check 4/5: File length cap (CONSTRAINTS.md §5: max 500 lines per file)"
VIOLATIONS_BEFORE_4="${VIOLATIONS}"
MAX_LINES=500
GRANDFATHERED=()
get_ceiling() {
  local path="$1"
  if [ "${#GRANDFATHERED[@]}" -gt 0 ]; then
    for entry in "${GRANDFATHERED[@]}"; do
      case "${entry}" in
        "${path}:"*) printf '%s' "${entry#*:}"; return 0 ;;
      esac
    done
  fi
  printf '%s' "${MAX_LINES}"
}
while IFS= read -r file; do
  [ -f "${file}" ] || continue
  case "${file}" in
    *.ts|*.tsx|*.js|*.jsx|*.mjs|*.cjs|*.py|*.go|*.rs) ;;
    *) continue ;;
  esac
  case "${file}" in
    */node_modules/*|*/dist/*|*/build/*|*.d.ts|*/__tests__/*|*.test.ts|*.test.tsx|*.test.js) continue ;;
    .agents/*|.claude/*|.codex/*|.cursor/*|.qwen/*|.gemini/*|.grok/*|.kimi-code/*) continue ;; # upstream-managed harness, not application source
    apps/dashboard/src/lib/canvas/*|*/apps/dashboard/src/lib/canvas/*) continue ;;  # visual canvas code, not subject to backend SRP
  esac
  lines=$(wc -l < "${file}" | tr -d ' ')
  ceiling="$(get_ceiling "${file}")"
  if [ "${lines}" -gt "${ceiling}" ]; then
    if [ "${ceiling}" = "${MAX_LINES}" ]; then
      violation "File exceeds ${MAX_LINES}-line cap: ${file} has ${lines} lines"
      violation "  Fix: Split by responsibility. See CONSTRAINTS.md §5 for an example decomposition."
    else
      violation "Grandfathered file grew above its ceiling: ${file} has ${lines} lines (ceiling ${ceiling})"
      violation "  Fix: Do not add to this file until it is split. See CONSTRAINTS.md §5."
    fi
    VIOLATIONS=$(( VIOLATIONS + 1 ))
  fi
done <<< "${FILES}"
[ "${VIOLATIONS}" -eq "${VIOLATIONS_BEFORE_4}" ] && ok "No oversized files found" || true
info "Check 5/5: Test coverage thresholds"
if [ "${SKIP_COVERAGE:-0}" = "1" ]; then
  warn "SKIP_COVERAGE=1 set — skipping coverage gate. CI must not skip this check."
else
  VIOLATIONS_BEFORE_5="${VIOLATIONS}"
  COVERAGE_WORKSPACE_ROOT="${WORKSPACE_ROOT:-}"
  if [ -z "${COVERAGE_WORKSPACE_ROOT}" ]; then
    COVERAGE_WORKSPACE_ROOT="$(mktemp -d -t av-validate-workspaces-XXXXXX)"
    export WORKSPACE_ROOT="${COVERAGE_WORKSPACE_ROOT}"
  fi
  if ! command -v bun >/dev/null 2>&1; then
    violation "bun not found on PATH — cannot run 'bun run test:coverage'."
    violation "  Fix: install bun (https://bun.sh) or set SKIP_COVERAGE=1 for local iteration only."
    VIOLATIONS=$(( VIOLATIONS + 1 ))
  else
    if ! bun run test:coverage; then
      violation "Tests or coverage check failed — see vitest output above."
      violation "  Fix: resolve reported test failures. If coverage is below threshold,"
      violation "       add tests for the reported uncovered lines/branches."
      violation "  Thresholds live in: vitest.config.ts"
      violation "  Design reference: docs/plans/v0-2-bigbang-design.md §5.6 / §7"
      VIOLATIONS=$(( VIOLATIONS + 1 ))
    fi
  fi
  [ "${VIOLATIONS}" -eq "${VIOLATIONS_BEFORE_5}" ] && ok "Coverage thresholds met" || true
fi
printf "\n"
if [ "${VIOLATIONS}" -eq 0 ]; then
  ok "All checks passed (0 violations)."
  exit 0
else
  violation "Total violations: ${VIOLATIONS}"
  violation "Fix all violations above and re-run: ./scripts/harness/validate.sh"
  exit 1
fi
