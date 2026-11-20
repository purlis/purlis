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

# The chat's calls go through `gh api` under the token sentinel GH_TOKEN_VIA_SHIM: purlis's `gh`
# shim supplies the chat's minted installation token, so the script never holds, prints or stores
# it. Every other token (the person's own, or a NARROW_TOKEN) is sent with curl.
GH_TOKEN_VIA_SHIM="@gh"

# shim_parse: set STATUS and BODY from the `gh api --include` reply in $WORK/raw.
shim_parse() {
  STATUS=$(awk 'NR==1 && /^HTTP/ {print $2}' "$WORK/raw")
  [[ -n "$STATUS" ]] || STATUS=000
  # The reply's body is what follows the header block's blank line.
  BODY=$(awk 'f {print; next} {sub(/\r$/, "")} $0 == "" {f=1}' "$WORK/raw")
  [[ "$STATUS" != 000 || -n "$BODY" ]] || BODY=$(head -c 200 "$WORK/gherr" | tr '\n' ' ')
}

# send TOKEN METHOD API_PATH [JSON]: one call to api.github.com (API_PATH starts with /). Sets
# STATUS and BODY, whichever transport the token selects.
send() {
  local tok=$1 m=$2 p=$3 d=${4-}
  if [[ "$tok" == "$GH_TOKEN_VIA_SHIM" ]]; then
    local args=(api --include -X "$m" -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28")
    [[ -n "$d" ]] && args+=(--input -)
    # gh exits non-zero on a 4xx/5xx but still prints the reply with --include, so its exit is ignored.
    if [[ -n "$d" ]]; then gh "${args[@]}" "${p#/}" <<<"$d" >"$WORK/raw" 2>"$WORK/gherr" || true
    else gh "${args[@]}" "${p#/}" >"$WORK/raw" 2>"$WORK/gherr" </dev/null || true; fi
    shim_parse
    return 0
  fi
  local args=(-sS -o "$WORK/body" -w '%{http_code}' -X "$m"
    -H "Authorization: Bearer $tok" -H "Accept: application/vnd.github+json"
    -H "X-GitHub-Api-Version: 2022-11-28")
  [[ -n "$d" ]] && args+=(-H "Content-Type: application/json" --data "$d")
  STATUS=$(curl "${args[@]}" "https://api.github.com$p") || STATUS=000
  BODY=$(cat "$WORK/body" 2>/dev/null || true)   # the whole reply: only `record` shortens
}

# req TOKEN METHOD PATH [JSON]: a call under /repos/$REPO. Sets STATUS and BODY.
req() {
  local tok=$1 m=$2 p=$3 d=${4-}
  [[ -n "${REPO-}" ]] || die "REPO not set"
  [[ -z "$p" || "$p" == /* ]] || die "path must start with /: $p"
  send "$tok" "$m" "/repos/$REPO$p" "$d"
}

# req_unscoped TOKEN METHOD PATH [JSON]: only the calls that cannot live under /repos/$REPO.
# The owner-specific routes are exactly the ones for $REPO's owner.
req_unscoped() {
  local tok=$1 m=$2 p=$3 d=${4-} owner=${REPO-}
  owner=${owner%%/*}   # GET /user runs before REPO is set
  case "$m $p" in
    "GET /user" | "POST /user/repos" | "GET /apps/"* | "POST /graphql") ;;
    "POST /orgs/$owner/repos" | "GET /user/memberships/orgs/$owner") [[ -n "$owner" ]] || die "REPO not set" ;;
    *) die "unscoped call not allowed: $m $p" ;;
  esac
  send "$tok" "$m" "$p" "$d"
}

# gql TOKEN QUERY: a GraphQL call; the query carries its values as literals. Under the shim the
# query goes as a `-f query=` field: purlis's `gh` refuses `--input` (file or stdin) on `graphql`.
gql() {
  if [[ "$1" == "$GH_TOKEN_VIA_SHIM" ]]; then
    gh api --include graphql -f query="$2" >"$WORK/raw" 2>"$WORK/gherr" </dev/null || true
    shim_parse
  else
    req_unscoped "$1" POST /graphql "$(jq -n --arg q "$2" '{query:$q}')"
  fi
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

# row_gql_refused TOKEN ROW EXPECTED QUERY: pass if GraphQL errored or returned null data, and no ref moved.
row_gql_refused() {
  local tok=$1 id=$2 exp=$3 before after res=fail obs
  before=$(refs_state "$tok")
  gql "$tok" "$4"
  local st=$STATUS bo=$BODY
  after=$(refs_state "$tok")
  STATUS=$st; BODY=$bo   # refs_state overwrote them
  if [[ "$STATUS" == 000 ]]; then res=inconclusive
  elif [[ "$STATUS" != 200 ]] || jq -e '(.errors | length > 0) or (.data == null) or ([.data[]?] | all(. == null))' <<<"$BODY" >/dev/null 2>&1; then res=pass; fi
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
