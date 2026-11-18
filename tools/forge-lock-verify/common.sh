#!/usr/bin/env bash
# shellcheck disable=SC2016  # jq filters are not shell variables
# Sourced by person.sh and chat.sh (#7133 slice 0). Not run on its own.
#
# One HTTP helper, one repo guard, one results recorder. Every request goes to
# /repos/$REPO..., and $REPO must be named purlis-forge-lock-verify-*. The only
# requests outside it are the allowlisted ones in req_unscoped.

VERIFY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS="${RESULTS:-$VERIFY_DIR/../../docs/research/forge-lock-verify-results.md}"
# shellcheck disable=SC2034  # used by the scripts that source this file
MARKER="purlis forge-lock verify: throwaway, safe to delete"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/forge-lock-verify.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
TOP_PID=$$

STATUS=""
BODY=""

# die MESSAGE: stop the whole run, also when called inside $(...) (a subshell's exit alone would not).
die() {
  printf 'error: %s\n' "$*" >&2
  (( BASH_SUBSHELL == 0 )) || kill -TERM "$TOP_PID" 2>/dev/null
  exit 1
}

# jqb ROW JQ_ARGS...: jq over the whole reply in BODY. A jq that fails (a reply that is not JSON,
# a filter that does not fit it) stops the whole run naming ROW, even from inside $(...), instead
# of returning an empty answer a decision would then read as "nothing there".
jqb() {
  local row=$1; shift
  jq "$@" <<<"$BODY" 2>"$WORK/jqerr" && return 0
  die "row $row: jq failed on the reply (HTTP $STATUS, ${#BODY} bytes): $(head -c 200 "$WORK/jqerr" | tr '\n' ' ')"
}

# need_repo_name NAME: refuse any repo not named purlis-forge-lock-verify-<suffix>.
need_repo_name() {
  [[ "$1" =~ ^purlis-forge-lock-verify-[A-Za-z0-9._-]+$ ]] \
    || die "repo '$1' is not named purlis-forge-lock-verify-*; refusing"
}

# need_repo OWNER/NAME sets REPO after the name check.
need_repo() {
  [[ "$1" == */* ]] || die "give the repo as owner/purlis-forge-lock-verify-<suffix>"
  need_repo_name "${1##*/}"
  REPO="$1"
}

# req TOKEN METHOD PATH [JSON]: a call under /repos/$REPO. Sets STATUS and BODY.
req() {
  local tok=$1 m=$2 p=$3 d=${4-}
  [[ -n "${REPO-}" ]] || die "REPO not set"
  [[ -z "$p" || "$p" == /* ]] || die "path must start with /: $p"
  local args=(-sS -o "$WORK/body" -w '%{http_code}' -X "$m"
    -H "Authorization: Bearer $tok" -H "Accept: application/vnd.github+json"
    -H "X-GitHub-Api-Version: 2022-11-28")
  [[ -n "$d" ]] && args+=(-H "Content-Type: application/json" --data "$d")
  STATUS=$(curl "${args[@]}" "https://api.github.com/repos/$REPO$p") || STATUS=000
  BODY=$(cat "$WORK/body" 2>/dev/null || true)   # the whole reply: only `record` shortens
}

# req_unscoped TOKEN METHOD PATH [JSON]: only the calls that cannot live under /repos/$REPO.
req_unscoped() {
  local tok=$1 m=$2 p=$3 d=${4-}
  case "$m $p" in
    "GET /user" | "POST /user/repos" | "GET /apps/"* | "POST /graphql") ;;
    *) die "unscoped call not allowed: $m $p" ;;
  esac
  local args=(-sS -o "$WORK/body" -w '%{http_code}' -X "$m"
    -H "Authorization: Bearer $tok" -H "Accept: application/vnd.github+json"
    -H "X-GitHub-Api-Version: 2022-11-28")
  [[ -n "$d" ]] && args+=(-H "Content-Type: application/json" --data "$d")
  STATUS=$(curl "${args[@]}" "https://api.github.com$p") || STATUS=000
  BODY=$(cat "$WORK/body" 2>/dev/null || true)   # the whole reply: only `record` shortens
}

# gql TOKEN QUERY VARS_JSON: a GraphQL call; the variables name the repo.
gql() {
  local payload
  payload=$(jq -n --arg q "$2" --argjson v "$3" '{query:$q,variables:$v}')
  req_unscoped "$1" POST /graphql "$payload"
}

# msg: the first message in BODY, one line, 160 characters.
msg() {
  jq -r '(.message // .errors[0].message // .errors[0] // "") | tostring' <<<"$BODY" 2>/dev/null \
    | tr '\n' ' ' | head -c 160 || true
}

observed() { printf 'HTTP %s %s' "$STATUS" "$(msg)"; }

esc() { local s=${1//$'\n'/ }; printf '%s' "${s//|/\\|}"; }

# begin_run SCRIPT PHASE ACTOR: a heading and table header in the results file.
begin_run() {
  [[ -f "$RESULTS" ]] || die "results file missing: $RESULTS"
  {
    printf '\n## %s · %s · %s · as %s · %s\n\n' "$(date -u +%Y-%m-%dT%H:%MZ)" "$1" "$2" "$3" "$REPO"
    printf '| row | expected | observed | result |\n|---|---|---|---|\n'
  } >>"$RESULTS"
}

# record ROW EXPECTED OBSERVED RESULT (pass, fail, inconclusive or info).
record() {
  printf '| %s | %s | %s | %s |\n' "$1" "$(esc "$2")" "$(esc "$3")" "$4" >>"$RESULTS"
  printf '%-8s %-13s %s\n' "$1" "$4" "$(esc "$3")"
}

is_refused() { case "$STATUS" in 401 | 403 | 404 | 405 | 409 | 422) return 0 ;; *) return 1 ;; esac; }

# refs_state: the head SHAs of every branch and tag, so a refused row can show the refs did not move.
refs_state() {
  local out="" part
  for part in heads tags; do
    req "$1" GET "/git/matching-refs/$part"
    out+=$(jqb "refs_state/$part" -r '[.[]? | "\(.ref)=\(.object.sha)"] | sort | join(",")')
    out+=";"
  done
  printf '%s' "$out"
}

# row_refused TOKEN ROW EXPECTED METHOD PATH [JSON]: pass only if refused AND no ref moved.
row_refused() {
  local tok=$1 id=$2 exp=$3 m=$4 p=$5 d=${6-} before after res=pass
  before=$(refs_state "$tok")
  req "$tok" "$m" "$p" "$d"
  local obs st=$STATUS bo=$BODY; obs=$(observed)
  after=$(refs_state "$tok")
  STATUS=$st; BODY=$bo   # refs_state overwrote them
  is_refused || res=fail
  if [[ "$before" != "$after" ]]; then res=fail; obs="$obs [a ref moved]"; fi
  record "$id" "$exp" "$obs" "$res"
}

# row_allowed TOKEN ROW EXPECTED "CODES" METHOD PATH [JSON]: pass if the status is one of CODES.
row_allowed() {
  local tok=$1 id=$2 exp=$3 codes=$4 m=$5 p=$6 d=${7-} res=fail
  req "$tok" "$m" "$p" "$d"
  [[ " $codes " == *" $STATUS "* ]] && res=pass
  record "$id" "$exp" "$(observed)" "$res"
}

# row_info TOKEN ROW EXPECTED METHOD PATH [JSON]: record what was seen; no verdict.
row_info() {
  req "$1" "$4" "$5" "${6-}"
  record "$2" "$3" "$(observed)" info
}

# row_gql_refused TOKEN ROW EXPECTED QUERY VARS: pass if GraphQL errored or returned null data, and no ref moved.
row_gql_refused() {
  local tok=$1 id=$2 exp=$3 before after res=fail obs
  before=$(refs_state "$tok")
  gql "$tok" "$4" "$5"
  local st=$STATUS bo=$BODY
  after=$(refs_state "$tok")
  STATUS=$st; BODY=$bo   # refs_state overwrote them
  if [[ "$STATUS" != 200 ]] || jq -e '(.errors | length > 0) or (.data == null) or ([.data[]?] | all(. == null))' <<<"$BODY" >/dev/null 2>&1; then res=pass; fi
  obs="HTTP $STATUS $(msg)"
  if [[ "$before" != "$after" ]]; then res=fail; obs="$obs [a ref moved]"; fi
  [[ "$res" == fail && "$obs" == "HTTP 200 " ]] && obs="HTTP 200 $(head -c 120 <<<"$BODY" | tr '\n' ' ')"
  record "$id" "$exp" "$obs" "$res"
}

# pr_num TOKEN HEAD_BRANCH: the number of the pull request whose head is HEAD_BRANCH.
pr_num() {
  req "$1" GET "/pulls?state=all&per_page=100"
  [[ "$STATUS" == 200 ]] || die "pr_num $2: GET /pulls failed: $(observed)"
  jqb "pr_num/$2" -r --arg h "$2" '[.[]? | select(.head.ref == $h) | .number][0] // empty'
}

b64() { printf '%s' "$1" | base64 | tr -d '\n'; }
