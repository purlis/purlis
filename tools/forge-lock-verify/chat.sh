#!/usr/bin/env bash
# Slice 0, the chat's half (#7133, design section 11): every row run as the chat's minted App
# token, each expected to be refused unless it says "allowed". Appends to
# docs/research/forge-lock-verify-results.md. Run from a chat, by a later task once the repo exists and the
# purlis agent App is installed on it (person.sh setup).
#
#   tools/forge-lock-verify/chat.sh <owner>/purlis-forge-lock-verify-<x> <phase>
#
#   A  before any ruleset: the controls (they must SUCCEED, or the later refusals prove nothing)
#   B  after `person.sh REPO lock`: every refused row, and the allowed rows
#   B2 after B: reruns only the rows B left inconclusive (P3, P8, P9, P10, M3 re-read, R6)
#   C  after `person.sh REPO automerge`: auto-merge, then a push
#   D  after `person.sh REPO bypass-app-on`, with NARROW_TOKEN: a narrowed token of a bypass App
#      (purlis does not mint a narrowed token yet, so without NARROW_TOKEN the row is skipped)
#
# Every chat call goes through `gh api`, run from a chat so purlis's `gh` shim supplies the chat's
# minted installation token: this script never holds, prints or stores it. Only this one repo is touched.
# shellcheck disable=SC2016  # GraphQL variables are not shell variables
set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

[[ $# -eq 2 ]] || die "usage: chat.sh <owner>/purlis-forge-lock-verify-<x> A|A2|B|B2|C|D"
need_repo "$1"
PHASE=$2
command -v gh >/dev/null 2>&1 || die "gh is needed (the one purlis's shim puts on a chat's PATH)"
TOKEN="$GH_TOKEN_VIA_SHIM"
req "$TOKEN" GET ""
[[ "$STATUS" == 200 ]] || die "cannot read $REPO through the chat's gh: $(observed)"
jqb marker -e --arg m "$MARKER" '.description == $m' >/dev/null \
  || die "$REPO does not carry the throwaway marker; refusing"
REPO_NODE=$(jqb repo -r .node_id)

main_sha() { req "$TOKEN" GET "/git/ref/heads/main"; jqb main_sha -r '.object.sha // empty'; }
SHA=$(main_sha)
[[ -n "$SHA" ]] || die "cannot read main"

phase_a() {
  begin_run chat.sh A chat
  local n; n=$(pr_num "$TOKEN" vf-m0)
  row_allowed "$TOKEN" M0 "allowed 200 (no ruleset: the test can pass)" "200" PUT "/pulls/$n/merge" '{"merge_method":"merge"}'
  row_allowed "$TOKEN" P5-ctl "allowed 201 (control: create refs/heads/rel/ctl)" "201" POST /git/refs \
    "{\"ref\":\"refs/heads/rel/ctl\",\"sha\":\"$(main_sha)\"}"
}

# A2: the ref-create control alone, for a repo whose vf-m0 is already merged (phase A ran once,
# and its first ref under release/ hit git's directory/file conflict with the release branch).
phase_a2() {
  begin_run chat.sh A2 chat
  row_allowed "$TOKEN" P5-ctl "allowed 201 (control: create refs/heads/rel/ctl)" "201" POST /git/refs \
    "{\"ref\":\"refs/heads/rel/ctl\",\"sha\":\"$(main_sha)\"}"
}

# P3: a force PATCH of main to a commit that is NOT main's head (the head of a feature branch), so
# a success would move main. A PATCH to main's own sha is a no-op and proves nothing.
row_p3() {
  local other cur
  cur=$(main_sha)
  for b in vf-allowed vf-w1 vf-merges; do
    req "$TOKEN" GET "/git/ref/heads/$b"; other=$(jqb P3 -r '.object.sha // empty')
    [[ -n "$other" && "$other" != "$cur" ]] && break
    other=""
  done
  if [[ -z "$other" ]]; then
    record P3 "refused (force-update main to another sha)" "no branch head differs from main's: nothing to move main to" inconclusive
    return 0
  fi
  row_refused "$TOKEN" P3 "refused (force-update main to $b's head ${other:0:7}, not main's)" PATCH /git/refs/heads/main "{\"sha\":\"$other\",\"force\":true}"
}

# P8: retarget a PR whose base is not main (vf-retarget, put back on nightly first) to main.
row_p8() {
  local n base
  n=$(pr_num "$TOKEN" vf-retarget)
  [[ -n "$n" ]] || { record P8 "refused (retarget PR base to main)" "no pull request from vf-retarget" inconclusive; return 0; }
  req "$TOKEN" GET "/pulls/$n"; base=$(jqb P8 -r .base.ref)
  if [[ "$base" == main ]]; then
    req "$TOKEN" PATCH "/pulls/$n" '{"base":"nightly"}'
    record P8-setup "info (PR $n based on main: put it on nightly first, control)" "$(observed)" info
    req "$TOKEN" GET "/pulls/$n"; base=$(jqb P8 -r .base.ref)
  fi
  if [[ "$base" == main ]]; then
    record P8 "refused (retarget PR base to main)" "PR $n could not be moved off main: nothing to retarget" inconclusive
    return 0
  fi
  row_refused "$TOKEN" P8 "refused (retarget PR $n from $base to main)" PATCH "/pulls/$n" '{"base":"main"}'
  req "$TOKEN" GET "/pulls/$n"
  record P8-after "info (PR $n's base after the PATCH)" "base $(jqb P8 -r .base.ref)" info
}

# P9/P10: GraphQL, values inlined as literals (purlis's gh refuses --input on graphql). A reply with no
# HTTP status (000) is inconclusive, never a pass.
row_p9() {
  row_gql_refused "$TOKEN" P9 "refused (GraphQL createCommitOnBranch on main)" \
    "mutation{createCommitOnBranch(input:{branch:{repositoryNameWithOwner:\"$REPO\",branchName:\"main\"},message:{headline:\"gql\"},expectedHeadOid:\"$1\",fileChanges:{additions:[{path:\"gql.txt\",contents:\"$(b64 x)\"}]}}){commit{oid}}}"
}
row_p10() {
  row_gql_refused "$TOKEN" P10 "refused (GraphQL mergeBranch into main)" \
    "mutation{mergeBranch(input:{repositoryId:\"$REPO_NODE\",base:\"main\",head:\"vf-merges\"}){mergeCommit{oid}}}"
}

# B2: only what B left inconclusive. M3 is re-read, not re-sent.
phase_b2() {
  begin_run chat.sh B2 chat
  local sha n rid
  sha=$(main_sha)
  row_p3
  row_p8
  row_p9 "$sha"
  row_p10
  n=$(pr_num "$TOKEN" vf-m3)
  sleep 20
  req "$TOKEN" GET "/pulls/$n"
  record M3-reread "info (PR $n after merge-async answered 202, re-read 20s on; main was ${SHA:0:7})" \
    "state $(jqb M3 -r .state) merged $(jqb M3 -r .merged) mergeable_state $(jqb M3 -r .mergeable_state); main now $(main_sha | cut -c1-7)" info
  req "$TOKEN" GET /releases/tags/v1
  if [[ "$STATUS" == 200 ]]; then
    rid=$(jqb R6 -r .id)
    row_allowed "$TOKEN" R6 "allowed (notes of the published release v1)" "200" PATCH "/releases/$rid" '{"body":"edited by chat"}'
  else
    req "$TOKEN" GET /releases/tags/v0
    rid=$(jqb R6 -r '.id // empty')
    if [[ -n "$rid" ]]; then
      row_allowed "$TOKEN" R6 "allowed (notes of published release v0; v1 is gone, and v0 is not immutable)" "200" PATCH "/releases/$rid" '{"body":"edited by chat"}'
    else
      record R6 "allowed (edit a published release)" "no published release exists" inconclusive
    fi
  fi
}

phase_b() {
  begin_run chat.sh B chat
  local n sha rs_id
  sha=$(main_sha)
  # Merges
  n=$(pr_num "$TOKEN" vf-m1); row_refused "$TOKEN" M1 "refused (merge)" PUT "/pulls/$n/merge" '{"merge_method":"merge"}'
  n=$(pr_num "$TOKEN" vf-m2); row_refused "$TOKEN" M2 "refused; allowed means the gate App is needed" PUT "/pulls/$n/merge" '{"merge_method":"squash"}'
  n=$(pr_num "$TOKEN" vf-m3); row_refused "$TOKEN" M3 "refused or 404 (endpoint may not exist)" PUT "/pulls/$n/merge-async" '{}'
  # Refs and contents
  row_refused "$TOKEN" P1 "refused (PUT /contents on main)" PUT /contents/x.txt \
    "{\"message\":\"m\",\"content\":\"$(b64 x)\",\"branch\":\"main\"}"
  row_refused "$TOKEN" P2 "refused (POST /merges into main)" POST /merges '{"base":"main","head":"vf-merges"}'
  row_p3
  row_refused "$TOKEN" P4 "refused (delete release)" DELETE /git/refs/heads/release
  row_refused "$TOKEN" P5 "refused (create refs/heads/rel/new)" POST /git/refs "{\"ref\":\"refs/heads/rel/new\",\"sha\":\"$sha\"}"
  row_refused "$TOKEN" P6 "refused (rename vf-rename into rel/x)" POST /branches/vf-rename/rename '{"new_name":"rel/x"}'
  row_refused "$TOKEN" P7 "refused (merge-upstream)" POST /merge-upstream '{"branch":"main"}'
  if [[ "$STATUS" == 422 ]] && grep -qi 'fork' <<<"$BODY"; then
    record P7-note "refusal must come from the ruleset" "not a fork: the refusal proves nothing about the lock" inconclusive
  fi
  row_p8
  row_p9 "$sha"
  row_p10
  # Tags
  row_allowed "$TOKEN" T0 "allowed 201 (tag object only, no ref)" "201" POST /git/tags \
    "{\"tag\":\"v9\",\"message\":\"m\",\"object\":\"$sha\",\"type\":\"commit\"}"
  row_refused "$TOKEN" T1 "refused (create refs/tags/v9)" POST /git/refs "{\"ref\":\"refs/tags/v9\",\"sha\":\"$sha\"}"
  row_refused "$TOKEN" T2 "refused (force-move refs/tags/v0)" PATCH /git/refs/tags/v0 "{\"sha\":\"$sha\",\"force\":true}"
  row_refused "$TOKEN" T3 "refused (delete refs/tags/v0)" DELETE /git/refs/tags/v0
  # Releases: expected refused (the lock's aim); design 6.2 predicts R2 to R4 allowed, so a fail is that gap.
  row_refused "$TOKEN" R1 "refused (release on a NEW tag v10)" POST /releases '{"tag_name":"v10","target_commitish":"main"}'
  row_refused "$TOKEN" R2 "refused (release on EXISTING tag v0; design predicts allowed)" POST /releases '{"tag_name":"v0","name":"chat"}'
  row_refused "$TOKEN" R3 "refused (draft release; design predicts allowed)" POST /releases '{"tag_name":"v0","draft":true,"name":"chat-draft"}'
  local rid=""
  if [[ "$STATUS" == 201 ]]; then rid=$(jqb R3 -r .id); fi
  if [[ -n "$rid" ]]; then
    row_refused "$TOKEN" R4 "refused (publish the draft; design predicts allowed)" PATCH "/releases/$rid" '{"draft":false}'
  else
    record R4 "refused (publish a draft)" "no draft exists to publish (R3 was refused)" pass
  fi
  req "$TOKEN" GET /releases/tags/v1
  local v1 asset
  v1=$(jqb R5 -r '.id // empty'); asset=$(jqb R5 -r '.assets[0].id // empty')
  if [[ -n "$v1" ]]; then
    [[ -n "$asset" ]] && row_refused "$TOKEN" R5a "refused (delete an asset of immutable release v1)" DELETE "/releases/assets/$asset"
    row_refused "$TOKEN" R5b "refused (delete immutable release v1)" DELETE "/releases/$v1"
    row_allowed "$TOKEN" R6 "allowed (notes of a published immutable release; design 6.2)" "200" PATCH "/releases/$v1" '{"body":"edited by chat"}'
  else
    record R5 "immutable release v1" "release v1 not found: person.sh setup did not finish" inconclusive
  fi
  # The lock itself
  req "$TOKEN" GET /rulesets
  rs_id=${RULESET_ID:-$(jqb L0 -r '[.[]? | select(.name == "purlis: protected branches") | .id][0] // empty')}
  record L0 "chat can list rulesets (info)" "$(observed)" info
  if [[ -n "$rs_id" ]]; then
    row_refused "$TOKEN" L1 "refused (PUT ruleset: disable the lock)" PUT "/rulesets/$rs_id" '{"enforcement":"disabled"}'
    row_refused "$TOKEN" L2 "refused (DELETE ruleset)" DELETE "/rulesets/$rs_id"
    row_info "$TOKEN" C1 "info (GET ruleset; is there a current_user_can_bypass field)" GET "/rulesets/$rs_id"
  else
    record L1 "refused (PUT ruleset)" "ruleset id unknown (list refused); set RULESET_ID and rerun" inconclusive
    record L2 "refused (DELETE ruleset)" "ruleset id unknown (list refused); set RULESET_ID and rerun" inconclusive
  fi
  row_refused "$TOKEN" L3 "refused (allow_auto_merge true)" PATCH "" '{"allow_auto_merge":true}'
  row_info "$TOKEN" C2 "info (rules at main, no admin)" GET /rules/branches/main
  row_refused "$TOKEN" D1 "refused (repository_dispatch; 204 is a CI-route gap)" POST /dispatches '{"event_type":"verify"}'
  # The allowed rows (design 12.4) and the CI route
  row_allowed "$TOKEN" AL1 "allowed (push a feature branch)" "201" POST /git/refs "{\"ref\":\"refs/heads/vf-allowed\",\"sha\":\"$sha\"}"
  row_allowed "$TOKEN" AL2 "allowed (edit own branch)" "201" PUT /contents/allowed.txt \
    "{\"message\":\"allowed\",\"content\":\"$(b64 ok)\",\"branch\":\"vf-allowed\"}"
  row_allowed "$TOKEN" AL3 "allowed (open a pull request)" "201" POST /pulls '{"title":"allowed","head":"vf-allowed","base":"main"}'
  n=$(jqb AL3 -r '.number // empty')
  [[ -n "$n" ]] && row_allowed "$TOKEN" AL4 "allowed (comment)" "201" POST "/issues/$n/comments" '{"body":"chat comment"}'
  ci_route "$sha"
}

# W1: push a branch editing the script ci.yml runs, to write main with the workflow's GITHUB_TOKEN.
ci_route() {
  local sha=$1 fsha
  row_allowed "$TOKEN" W1a "allowed (create branch vf-w1)" "201" POST /git/refs "{\"ref\":\"refs/heads/vf-w1\",\"sha\":\"$sha\"}"
  req "$TOKEN" GET "/contents/scripts/run.sh?ref=vf-w1"; fsha=$(jqb W1b -r '.sha // empty')
  local script
  script=$(printf '#!/usr/bin/env bash\nset -eu\ncurl -fsS -X PUT -H "Authorization: Bearer $GITHUB_TOKEN" -d %s "https://api.github.com/repos/$GITHUB_REPOSITORY/contents/ci-wrote.txt"\n' \
    "'{\"message\":\"ci\",\"content\":\"eA==\",\"branch\":\"main\"}'")
  row_allowed "$TOKEN" W1b "allowed (edit scripts/run.sh on own branch; CI run then reported by person.sh press)" "200" PUT /contents/scripts/run.sh \
    "$(jq -n --arg c "$(b64 "$script")" --arg s "$fsha" '{message:"edit script",content:$c,sha:$s,branch:"vf-w1"}')"
}

# A1: enable auto-merge on vf-auto, then push; the merge must not happen.
phase_c() {
  begin_run chat.sh C chat
  local n node i merged="false" by=""
  n=$(pr_num "$TOKEN" vf-auto)
  [[ -n "$n" ]] || die "no pull request from vf-auto"
  req "$TOKEN" GET "/pulls/$n"; node=$(jqb A1-enable -r .node_id)
  gql "$TOKEN" "mutation{enablePullRequestAutoMerge(input:{pullRequestId:\"$node\",mergeMethod:MERGE}){pullRequest{autoMergeRequest{enabledBy{login}}}}}"
  record A1-enable "info (enable permitted or refused)" "HTTP $STATUS $(msg) $(jqb A1-enable -c '.data // empty' | head -c 120)" info
  req "$TOKEN" GET "/contents/vf-auto.txt?ref=vf-auto"
  req "$TOKEN" PUT /contents/vf-auto-2.txt "{\"message\":\"push after auto-merge\",\"content\":\"$(b64 again)\",\"branch\":\"vf-auto\"}"
  record A1-push "allowed (the chat's push to its own branch)" "$(observed)" info
  for i in $(seq 1 18); do
    sleep 10
    req "$TOKEN" GET "/pulls/$n"
    merged=$(jqb A1-merge -r .merged); by=$(jqb A1-merge -r '.merged_by.login // ""')
    [[ "$merged" == true ]] && break
  done
  if [[ "$merged" == true ]]; then
    record A1-merge "PR not merged (the lock holds)" "merged by '$by' within $((i * 10))s: auto-merge completed past the lock" fail
  else
    record A1-merge "PR not merged (the lock holds)" "not merged after 180s (mergeable_state $(jqb A1-merge -r .mergeable_state))" pass
  fi
}

# B1: a narrowed token of the App while the App is a bypass actor.
phase_d() {
  begin_run chat.sh D narrowed-token
  local nt="${NARROW_TOKEN:-}"
  if [[ -z "$nt" ]]; then
    record B1 "allowed (needs a token narrowed to contents:write; purlis offers no narrowed mint yet)" "NARROW_TOKEN not set: skipped" info
    return 0
  fi
  row_allowed "$nt" B1 "allowed (hypothesis: an App bypass actor bypasses whatever the token's scope)" "200 201" PUT /contents/b1.txt \
    "{\"message\":\"b1\",\"content\":\"$(b64 b1)\",\"branch\":\"main\"}"
}

case "$PHASE" in
  A) phase_a ;;
  A2) phase_a2 ;;
  B) phase_b ;;
  B2) phase_b2 ;;
  C) phase_c ;;
  D) phase_d ;;
  *) die "phase must be A, A2, B, B2, C or D" ;;
esac
printf 'rows appended to %s\n' "$RESULTS"
