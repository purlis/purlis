#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# One login, slice I: the LIVE walk of ADR 0128's scenario, with your own accounts. Not run in CI.
#
#   tools/one-login-live.sh [--github OWNER/REPO] [--gitlab GROUP/PROJECT]
#                           [--ghes HOST OWNER/REPO] [--gitlab-host HOST GROUP/PROJECT]
#                           [--other-repo github|gitlab OWNER/REPO] [--expect-owner HOST NAME]
#                           [--client-id HOST ID] [--probe-refusals] [--probes-only]
#
# It does what `tests/one-login-e2e/run.mjs` does against a fake forge, against real ones, and
# prints PASS, FAIL or SKIP for every step:
#
#   probes    the two gates ADR 0128 names (Open item 3), read from the live forge:
#             1. the OAuth app's access tokens EXPIRE and a refresh works (GitHub: the registration's
#                "Expire user access tokens" is on); for GitLab the same, read from the reply;
#             2. the consent page shows who registered the application (GHES, GitLab).
#   scenario  sign in once, clone as a project, then a chat's gh, glab and git: pr/mr create, list
#             and view, `gh api`, fetch, pull and a push to a branch it made; and what must be
#             refused: a push to main, `gh pr merge` (it becomes an ask), another repo, `gh auth
#             token`; the renewal; and that the chat never saw a token.
#
# **No credential is ever printed, and none is written anywhere but your keyring.** A token lives in
# shell variables and in pipes (never in an argument list, which `ps` shows), and in the OS keyring
# (macOS Keychain, or libsecret's `secret-tool` on Linux) under the service `purlis-one-login-live`,
# which the script deletes when it ends. It never uses your own `gh`, `glab` or git login.
#
# **The scenario steps need the slices of ADR 0128 that are not merged yet.** Each is driven through
# a command you give in the environment, and a step with no driver is SKIPPED, never passed:
#
#   PURLIS_LIVE_SIGN_IN    sign in to a host            `{host}`                          TODO(slice G)
#   PURLIS_LIVE_CLONE      clone a repo as a project    `{host}` `{repo}` `{dest}`        TODO(slice D)
#   PURLIS_LIVE_CHAT       run a command as the chat    prefix; the command follows, with
#                          its working directory a project's clone                          TODO(slice B)
#   PURLIS_LIVE_CHAT_DIR   print the folder a chat can read (its home and clones)         TODO(slice B)
#   PURLIS_LIVE_ASKS       print the pending asks, one per line                           TODO(slice F)
#   PURLIS_LIVE_RENEW      make the stored token due for renewal now                      TODO(slice C)
#   PURLIS_LIVE_TOKEN_ITEMS print, one per line, `service<TAB>account` of each keyring item purlis
#                          stored a token in, so the scan looks for THOSE tokens        TODO(slice C)
#   PURLIS_LIVE_AUDIT      print the proxy's audit, one line per request                  TODO(slice A2)
#
# Refusals that would change a real repository if purlis failed to refuse them (a push to main,
# `gh pr merge`) run only with `--probe-refusals`: use a scratch repo whose main is protected.
# The script makes a branch `purlis-live-<time>` and a pull or merge request. At the end it closes
# them and deletes the branch and tag itself, with the token of its own probe sign-in (a chat may not,
# by design), revokes the probe tokens where the forge allows it (GitLab; GitHub's needs the app's
# client secret, so it prints the page to revoke on), and removes its temporary clones.

set -u -o pipefail

SERVICE=purlis-one-login-live
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REGISTRATION="$HERE/../crates/purlis-core/src/forge/registration.rs"

pass=0 fail=0 skip=0
declare -a CLIENT_IDS=() OWNERS=() PROBED=()
github_repo="" gitlab_repo="" ghes_host="" ghes_repo="" glhost="" glhost_repo=""
other_kind="" other_repo="" probe_refusals=0 probes_only=0
branch="purlis-live-$(date +%Y%m%d%H%M%S)"

# ---- reporting ------------------------------------------------------------------------------------

step_pass() { pass=$((pass + 1)); printf 'PASS  %s\n' "$1"; }
step_fail() { fail=$((fail + 1)); printf 'FAIL  %s\n        %s\n' "$1" "${2:-}"; }
step_skip() { skip=$((skip + 1)); printf 'SKIP  %s\n        %s\n' "$1" "$2"; }

die() { printf 'one-login-live: %s\n' "$1" >&2; exit 2; }

usage() { sed -n '4,17p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

# ---- arguments ------------------------------------------------------------------------------------

while [ $# -gt 0 ]; do
  case "$1" in
    --github) github_repo="${2:?}"; shift 2 ;;
    --gitlab) gitlab_repo="${2:?}"; shift 2 ;;
    --ghes) ghes_host="${2:?}"; ghes_repo="${3:?}"; shift 3 ;;
    --gitlab-host) glhost="${2:?}"; glhost_repo="${3:?}"; shift 3 ;;
    --other-repo) other_kind="${2:?}"; other_repo="${3:?}"; shift 3 ;;
    --expect-owner) OWNERS+=("${2:?}=${3:?}"); shift 3 ;;
    --client-id) CLIENT_IDS+=("${2:?}=${3:?}"); shift 3 ;;
    --probe-refusals) probe_refusals=1; shift ;;
    --probes-only) probes_only=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1 (see --help)" ;;
  esac
done
[ -n "$github_repo$gitlab_repo$ghes_host$glhost" ] || { usage; die "name at least one repo"; }

for tool in curl node git; do command -v "$tool" >/dev/null 2>&1 || die "$tool is needed"; done

# ---- the keyring, and only the keyring -------------------------------------------------------------

case "$(uname -s)" in
  Darwin) KEYRING=macos ;;
  Linux) command -v secret-tool >/dev/null 2>&1 && KEYRING=libsecret || die "secret-tool (libsecret-tools) is needed to keep a token off the disk" ;;
  *) die "this script needs the macOS Keychain or libsecret" ;;
esac

keyring_set() { # account; the secret is on stdin
  local secret
  secret="$(cat)"
  if [ "$KEYRING" = macos ]; then
    printf 'add-generic-password -U -s "%s" -a "%s" -w "%s"\n' "$SERVICE" "$1" "$secret" | security -i >/dev/null 2>&1
  else
    printf '%s' "$secret" | secret-tool store --label="$SERVICE $1" service "$SERVICE" account "$1" >/dev/null 2>&1
  fi
}
keyring_get() {
  if [ "$KEYRING" = macos ]; then security find-generic-password -s "$SERVICE" -a "$1" -w 2>/dev/null
  else secret-tool lookup service "$SERVICE" account "$1" 2>/dev/null; fi
}
keyring_del() {
  if [ "$KEYRING" = macos ]; then security delete-generic-password -s "$SERVICE" -a "$1" >/dev/null 2>&1
  else secret-tool clear service "$SERVICE" account "$1" >/dev/null 2>&1; fi
  return 0
}
declare -a TMPDIRS=() MADE=()
api_base() { # host kind
  if [ "$2" = github ]; then if [ "$1" = github.com ]; then printf 'https://api.github.com'; else printf 'https://%s/api/v3' "$1"; fi
  else printf 'https://%s/api/v4' "$1"; fi
}
# `call <method> <url> <token on stdin>`: the status code only. The token goes by `-H @-`.
call() { { printf 'Authorization: Bearer '; cat; printf '\n'; } | curl -sS -o /dev/null -w '%{http_code}' --max-time 30 -X "$1" -H @- -H 'Accept: application/json' "$2" 2>/dev/null; }
# Close what the scenario opened and delete its branch and tag, with the probe's own token.
remote_cleanup() { # host kind repo branch
  local host="$1" kind="$2" repo="$3" br="$4" base num enc
  base="$(api_base "$host" "$kind")"; enc="${repo//\//%2F}"
  keyring_get "$host:access" >/dev/null || return 0
  if [ "$kind" = github ]; then
    num="$(keyring_get "$host:access" | authed_get "$base/repos/$repo/pulls?state=open&head=${repo%%/*}:$br" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{process.stdout.write(String(JSON.parse(s)[0].number))}catch{}})')"
    [ -n "$num" ] && keyring_get "$host:access" | call PATCH "$base/repos/$repo/pulls/$num?state=closed" >/dev/null
    keyring_get "$host:access" | call DELETE "$base/repos/$repo/git/refs/heads/$br" >/dev/null
    keyring_get "$host:access" | call DELETE "$base/repos/$repo/git/refs/tags/$br" >/dev/null
  else
    num="$(keyring_get "$host:access" | authed_get "$base/projects/$enc/merge_requests?state=opened&source_branch=$br" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{process.stdout.write(String(JSON.parse(s)[0].iid))}catch{}})')"
    [ -n "$num" ] && keyring_get "$host:access" | call PUT "$base/projects/$enc/merge_requests/$num?state_event=close" >/dev/null
    keyring_get "$host:access" | call DELETE "$base/projects/$enc/repository/branches/$br" >/dev/null
    keyring_get "$host:access" | call DELETE "$base/projects/$enc/repository/tags/$br" >/dev/null
  fi
}
# Revoke a probe token where the forge lets a public client do it (GitLab); say where, otherwise.
revoke_probe() { # host kind
  local host="$1" kind="$2" cid t
  cid="$(client_id_for "$host" "$kind")"
  if [ "$kind" = gitlab ] && [ -n "$cid" ]; then
    for a in access refresh; do
      t="$(keyring_get "$host:$a")"; [ -n "$t" ] || continue
      printf 'client_id=%s&token=%s' "$cid" "$t" | post "https://$host/oauth/revoke" >/dev/null
    done
    printf 'revoked the probe tokens on %s\n' "$host"
  else
    printf 'REVOKE BY HAND: the probe signed in to %s; its token cannot be revoked without the app\x27s client secret. Remove the authorization at https://%s/settings/applications\n' "$host" "$host"
  fi
}
cleanup() {
  local m h k
  for m in "${MADE[@]:-}"; do [ -n "$m" ] || continue; read -r h k r b <<<"$m"; remote_cleanup "$h" "$k" "$r" "$b"; done
  for h in "${PROBED[@]:-}"; do
    [ -n "$h" ] || continue
    k=github; [[ " $glhost gitlab.com " == *" $h "* ]] && k=gitlab
    revoke_probe "$h" "$k"
    for a in access refresh; do keyring_del "$h:$a"; done
  done
  for d in "${TMPDIRS[@]:-}"; do [ -n "$d" ] && rm -rf "$d"; done
}
trap cleanup EXIT

# ---- a reply's JSON, read without printing it ------------------------------------------------------

# `jget <key>` reads JSON on stdin and prints that top-level key's value (nothing if absent).
jget() { node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{const v=JSON.parse(s)[process.argv[1]];if(v!==undefined&&v!==null)process.stdout.write(String(v))}catch{}})' "$1"; }

# `post <url> <form body on stdin>`: the reply on stdout. The body never appears in an argument.
post() { curl -sS --max-time 30 -H 'Accept: application/json' --data-binary @- "$1" 2>/dev/null; }

# `authed_get <url> <token on stdin>`: the reply's body. The token goes by `-H @-`, never as an argument.
authed_get() { { printf 'Authorization: Bearer '; cat; printf '\n'; } | curl -sS --max-time 30 -H @- -H 'Accept: application/json' "$1" 2>/dev/null; }

client_id_for() { # host kind
  local e
  for e in "${CLIENT_IDS[@]:-}"; do [ "${e%%=*}" = "$1" ] && { printf '%s' "${e#*=}"; return; }; done
  if [ -f "$REGISTRATION" ]; then
    if [ "$2" = github ] && [ "$1" = github.com ]; then sed -n 's/.*client_id: "\(Ov23[A-Za-z0-9]*\)".*/\1/p' "$REGISTRATION" | head -1; return; fi
    if [ "$2" = gitlab ] && [ "$1" = gitlab.com ]; then sed -n 's/.*client_id: "\([0-9a-f]\{64\}\)".*/\1/p' "$REGISTRATION" | head -1; return; fi
  fi
}
owner_for() { local e; for e in "${OWNERS[@]:-}"; do [ "${e%%=*}" = "$1" ] && { printf '%s' "${e#*=}"; return; }; done; }

open_url() { command -v open >/dev/null 2>&1 && open "$1" 2>/dev/null || { command -v xdg-open >/dev/null 2>&1 && xdg-open "$1" 2>/dev/null; } || true; }

# ---- the probes: what the live forge says --------------------------------------------------------------

# probe <host> <github|gitlab>: the device flow once, the reply's expiry and refresh read, the refresh
# tried without a secret, the account and (GitHub) the granted scopes read, and the consent page checked
# by you. The tokens go to the keyring and are deleted when the script ends.
probe() {
  local host="$1" kind="$2" cid dev_url tok_url dc uc vu resp tok="" ref="" newref="" exp="" err owner who
  cid="$(client_id_for "$host" "$kind")"
  if [ -z "$cid" ]; then step_skip "[$host] probes" "no client id: pass --client-id $host <id> (the registration an owner made for purlis on this host)"; return 1; fi
  if [ "$kind" = github ]; then dev_url="https://$host/login/device/code"; tok_url="https://$host/login/oauth/access_token"
  else dev_url="https://$host/oauth/authorize_device"; tok_url="https://$host/oauth/token"; fi
  local scope="api"; [ "$kind" = github ] && scope="repo read:org"
  resp="$(printf 'client_id=%s&scope=%s' "$cid" "${scope// /%20}" | post "$dev_url")"
  dc="$(printf '%s' "$resp" | jget device_code)"; uc="$(printf '%s' "$resp" | jget user_code)"; vu="$(printf '%s' "$resp" | jget verification_uri)"
  if [ -z "$dc" ]; then step_fail "[$host] the device flow starts" "no device code came back: is *Enable Device Flow* ticked for this registration?"; return 1; fi
  step_pass "[$host] the device flow starts"
  printf '\n  Open %s and enter the code %s.\n' "$vu" "$uc"
  open_url "$vu"
  owner="$(owner_for "$host")"
  if [ "$host" != github.com ] || [ -n "$owner" ]; then
    read -r -p "  On that page, does it show who registered the application${owner:+ (expected: $owner)}? [y/N] " who
    if [ "${who:-n}" = y ] || [ "${who:-n}" = Y ]; then step_pass "[$host] the consent page shows the registrant"
    else step_fail "[$host] the consent page shows the registrant" "ADR 0128 D1.3: a host whose page does not show the owner is a token connection, not a sign-in"; fi
  fi
  for _ in $(seq 1 60); do
    sleep 5
    resp="$(printf 'client_id=%s&device_code=%s&grant_type=urn:ietf:params:oauth:grant-type:device_code' "$cid" "$dc" | post "$tok_url")"
    tok="$(printf '%s' "$resp" | jget access_token)"
    [ -n "$tok" ] && break
    err="$(printf '%s' "$resp" | jget error)"
    case "$err" in authorization_pending|slow_down|"") ;; *) step_fail "[$host] the person's approval" "the forge answered: $err"; return 1 ;; esac
  done
  [ -n "$tok" ] || { step_fail "[$host] the person's approval" "timed out waiting for the approval"; return 1; }
  PROBED+=("$host")
  printf '%s' "$tok" | keyring_set "$host:access"
  if [ "$(keyring_get "$host:access")" = "$tok" ]; then step_pass "[$host] signed in; the token is in the keyring (read back, equal)"
  else step_fail "[$host] the token is in the keyring" "the keyring did not return what was written: nothing below can be trusted"; return 1; fi
  ref="$(printf '%s' "$resp" | jget refresh_token)"; exp="$(printf '%s' "$resp" | jget expires_in)"
  [ -n "$ref" ] && printf '%s' "$ref" | keyring_set "$host:refresh"
  if [ -n "$exp" ] && [ -n "$ref" ]; then step_pass "[$host] access tokens expire (${exp}s) and a refresh token comes with them"
  elif [ "$kind" = github ] && [[ "$cid" == Ov23* ]]; then
    step_fail "[$host] access tokens expire and a refresh token comes with them" "GATING: this client id (Ov23...) is a GitHub OAuth App. An OAuth App's tokens do not expire and come with no refresh token; *Expire user access tokens* is a GitHub App setting, not an OAuth App's. Renewal on GitHub cannot be shown with this registration (ADR 0128 D2, Open item 3): decide between a GitHub App registration and no renewal."
  else step_fail "[$host] access tokens expire and a refresh token comes with them" "none came back: the registration does not issue expiring tokens, so renewal cannot be shown"; fi
  if [ -n "$ref" ]; then
    resp="$(printf 'client_id=%s&grant_type=refresh_token&refresh_token=%s' "$cid" "$ref" | post "$tok_url")"
    newref="$(printf '%s' "$resp" | jget refresh_token)"
    if [ -n "$(printf '%s' "$resp" | jget access_token)" ] && [ -n "$newref" ] && [ "$newref" != "$ref" ]; then
      printf '%s' "$resp" | jget access_token | keyring_set "$host:access"
      printf '%s' "$newref" | keyring_set "$host:refresh"
      if [ "$(keyring_get "$host:refresh")" = "$newref" ]; then step_pass "[$host] a refresh works with no client secret, returns a NEW refresh token, and it is in the keyring"
      else step_fail "[$host] the rotated refresh token is in the keyring" "the keyring did not return what was written"; fi
      resp="$(printf 'client_id=%s&grant_type=refresh_token&refresh_token=%s' "$cid" "$ref" | post "$tok_url")"
      if [ -z "$(printf '%s' "$resp" | jget access_token)" ]; then step_pass "[$host] the used refresh token is refused (single use)"
      else step_fail "[$host] the used refresh token is refused (single use)" "the forge honoured it a second time"; fi
    elif [ -n "$(printf '%s' "$resp" | jget access_token)" ]; then step_fail "[$host] a refresh returns a new refresh token" "the refresh worked but returned no new refresh token (or the same one): ADR 0128 D2 relies on rotation"
    else step_fail "[$host] a refresh works with no client secret" "the forge answered: $(printf '%s' "$resp" | jget error) (the sign-in is a public client: ADR 0128 D2)"; fi
  fi
  local api="https://$host/api/v3/user" field=login
  [ "$host" = github.com ] && api="https://api.github.com/user"
  [ "$kind" = gitlab ] && { api="https://$host/api/v4/user"; field=username; }
  who="$(keyring_get "$host:access" | authed_get "$api" | jget "$field")"
  if [ -n "$who" ]; then step_pass "[$host] the new token reads the account ($who)"; else step_fail "[$host] the new token reads the account" "no account came back"; fi
  return 0
}

# ---- the scenario, through drivers ---------------------------------------------------------------------

need() { # <driver var> <slice> <step>: 0 when the driver is set; else a SKIP
  if [ -n "${!1:-}" ]; then return 0; fi
  step_skip "$3" "TODO($2): set $1 to the command that does this once that slice has merged"
  return 1
}
fill() { local s="$1"; shift; while [ $# -gt 1 ]; do s="${s//\{$1\}/$2}"; shift 2; done; printf '%s' "$s"; }

chat() { # the command, in a project's clone: the driver's prefix, then the command
  # shellcheck disable=SC2086
  ( cd "$CLONE" && $PURLIS_LIVE_CHAT "$@" )
}

expect_ok() { # <step> <cmd...>
  local name="$1" out; shift
  if out="$(chat "$@" 2>&1)"; then step_pass "$name"; return 0; fi
  step_fail "$name" "$(printf '%s' "$out" | tail -3 | tr '\n' ' ' | cut -c1-300)"; return 1
}
# A refusal counts only when it is purlis's: its wording in what the client printed, or (with the audit
# driver) a refusal the proxy recorded since. A forge's own 403 or 404 proves nothing about purlis.
expect_refused() {
  local name="$1" out before after; shift
  before=0; [ -n "${PURLIS_LIVE_AUDIT:-}" ] && before="$($PURLIS_LIVE_AUDIT 2>/dev/null | grep -ci 'refus\|held')"
  if out="$(chat "$@" 2>&1)"; then step_fail "$name" "it was NOT refused"; return 1; fi
  after=0; [ -n "${PURLIS_LIVE_AUDIT:-}" ] && after="$($PURLIS_LIVE_AUDIT 2>/dev/null | grep -ci 'refus\|held')"
  if printf '%s' "$out" | grep -qi 'purlis' || [ "$after" -gt "$before" ]; then step_pass "$name (refused by purlis: $(printf '%s' "$out" | tail -1 | cut -c1-120))"
  else step_fail "$name" "it failed, but not in purlis's words and with no refusal in the audit: the forge may have refused it, which proves nothing about purlis. Output: $(printf '%s' "$out" | tail -1 | cut -c1-160)"; fi
}

scenario() { # <host> <github|gitlab> <repo>
  local host="$1" kind="$2" repo="$3" dest
  printf '\n== scenario: %s %s ==\n' "$host" "$repo"
  need PURLIS_LIVE_SIGN_IN "slice G" "[$host] sign in once" && {
    read -ra cmd <<<"$(fill "$PURLIS_LIVE_SIGN_IN" host "$host")"
    if "${cmd[@]}" >/dev/null 2>&1; then step_pass "[$host] sign in once"; else step_fail "[$host] sign in once" "the sign-in driver failed"; return; fi
  } || return
  dest="$(mktemp -d)/clone"
  need PURLIS_LIVE_CLONE "slice D" "[$host] clone $repo as a project, recorded" || return
  read -ra cmd <<<"$(fill "$PURLIS_LIVE_CLONE" host "$host" repo "$repo" dest "$dest")"
  if "${cmd[@]}" >/dev/null 2>&1; then step_pass "[$host] clone $repo as a project, recorded"; else step_fail "[$host] clone $repo as a project, recorded" "the clone driver failed"; return; fi
  CLONE="$dest"; TMPDIRS+=("$(dirname "$dest")"); MADE+=("$host $kind $repo $branch")
  need PURLIS_LIVE_CHAT "slice B" "[$host] the chat's commands" || return

  expect_ok "[$host] git: a branch the chat creates" sh -c "git checkout -q -b $branch && echo live > purlis-live.txt && git add purlis-live.txt && git commit -q -m 'purlis one-login live check'" || return
  expect_ok "[$host] git push to that branch" git push -u origin "$branch" || return
  if [ "$kind" = github ]; then
    expect_ok "[$host] gh pr create" gh pr create --head "$branch" --base "$(git -C "$CLONE" symbolic-ref --short refs/remotes/origin/HEAD | sed 's#^origin/##')" --title "purlis one-login live check" --body "Made by tools/one-login-live.sh. Close it and delete the branch."
    expect_ok "[$host] gh pr list" gh pr list
    expect_ok "[$host] gh pr view" gh pr view "$branch"
    expect_ok "[$host] gh api on the repo" gh api "repos/$repo"
  else
    expect_ok "[$host] glab mr create" glab mr create --source-branch "$branch" --title "purlis one-login live check" --description "Made by tools/one-login-live.sh. Close it and delete the branch." --yes
    expect_ok "[$host] glab mr list" glab mr list
    expect_ok "[$host] glab mr view" glab mr view "$branch"
    expect_ok "[$host] glab api on the project" glab api "projects/${repo//\//%2F}"
  fi
  expect_ok "[$host] git fetch" git fetch origin
  expect_ok "[$host] git pull" git pull --ff-only origin "$(git -C "$CLONE" symbolic-ref --short refs/remotes/origin/HEAD | sed 's#^origin/##')"
  expect_ok "[$host] git push to the chat's own branch again" git push origin "$branch"

  if [ "$kind" = github ]; then expect_refused "[$host] gh auth token" gh auth token; fi
  if [ -n "$other_repo" ] && [ "$other_kind" = "$kind" ]; then
    if [ "$kind" = github ]; then expect_refused "[$host] another repo ($other_repo)" gh api "repos/$other_repo"
    else expect_refused "[$host] another repo ($other_repo)" glab api "projects/${other_repo//\//%2F}"; fi
    expect_refused "[$host] git ls-remote another repo" git ls-remote "https://$host/$other_repo.git"
  else step_skip "[$host] another repo" "pass --other-repo $kind OWNER/REPO, a repo you can reach that is not this project"; fi
  if [ "$probe_refusals" = 1 ]; then
    expect_refused "[$host] git push to the default branch" git push origin "HEAD:$(git -C "$CLONE" symbolic-ref --short refs/remotes/origin/HEAD | sed 's#^origin/##')"
    expect_refused "[$host] git push of a tag" git push origin "HEAD:refs/tags/$branch"
    if [ "$kind" = github ]; then expect_refused "[$host] gh pr merge (it becomes a held ask)" gh pr merge "$branch" --merge
    else expect_refused "[$host] glab mr merge (it becomes a held ask)" glab mr merge "$branch" --yes; fi
    if need PURLIS_LIVE_ASKS "slice F" "[$host] the merge is in Asks"; then
      if $PURLIS_LIVE_ASKS 2>/dev/null | grep -q "$branch"; then step_pass "[$host] the merge is in Asks"; else step_fail "[$host] the merge is in Asks" "no ask names $branch"; fi
    fi
  else step_skip "[$host] a push to the default branch, a tag, and a merge" "these would change the repo if purlis failed to refuse; re-run with --probe-refusals on a scratch repo"; fi

  if need PURLIS_LIVE_RENEW "slice C" "[$host] the token renews unattended"; then
    # shellcheck disable=SC2086
    $PURLIS_LIVE_RENEW "$host" >/dev/null 2>&1
    if [ "$kind" = github ]; then expect_ok "[$host] gh pr list after the renewal" gh pr list; else expect_ok "[$host] glab mr list after the renewal" glab mr list; fi
  fi
  PROJECTS+=("$host $repo $branch")
}

# The tokens to look for: the probe's, and the ones purlis itself stored (its own keyring items).
stored_tokens() { # prints one token per line, into a pipe: never to a terminal or a file
  local h a svc acct
  for h in "${SIGNED[@]:-}"; do for a in access refresh; do keyring_get "$h:$a"; echo; done; done
  [ -n "${PURLIS_LIVE_TOKEN_ITEMS:-}" ] || return 0
  while IFS=$'\t' read -r svc acct; do
    [ -n "$svc" ] || continue
    if [ "$KEYRING" = macos ]; then security find-generic-password -s "$svc" -a "$acct" -w 2>/dev/null
    else secret-tool lookup service "$svc" account "$acct" 2>/dev/null; fi
    echo
  done < <($PURLIS_LIVE_TOKEN_ITEMS 2>/dev/null)
}

chat_saw_no_token() {
  if ! need PURLIS_LIVE_CHAT_DIR "slice B" "the chat never saw a token (its files, its environment)"; then return; fi
  if [ -z "${PURLIS_LIVE_TOKEN_ITEMS:-}" ]; then step_skip "the scan covers the tokens purlis itself stored" "TODO(slice C): set PURLIS_LIVE_TOKEN_ITEMS; without it only this script's probe tokens are searched for, which proves nothing about purlis's"; fi
  local dir envout
  dir="$($PURLIS_LIVE_CHAT_DIR 2>/dev/null)"
  [ -d "$dir" ] || { step_fail "the chat never saw a token" "the chat-dir driver printed no folder"; return; }
  # The secrets go to grep on stdin as a pattern file: never on a command line. Empty lines are dropped.
  if stored_tokens | grep -v '^$' | grep -rqF -f /dev/stdin "$dir" 2>/dev/null; then step_fail "the chat never saw a token (its files)" "a stored token appears in a file the chat can read"
  else step_pass "the chat never saw a token (its files)"; fi
  if ! envout="$(chat env 2>&1)"; then step_fail "the chat never saw a token (its environment)" "could not read the chat's environment: $(printf '%s' "$envout" | tail -1 | cut -c1-120)"; return; fi
  if printf '%s' "$envout" | grep -qE '(gho_|ghp_|ghu_|ghr_|ghs_|github_pat_|glpat-|gloas-|glcbt-|gldt-|glrt-)[A-Za-z0-9_-]{16,}'; then step_fail "the chat never saw a token (its environment)" "a token-shaped value is in the chat's environment"
  elif printf '%s' "$envout" | stored_tokens_match; then step_fail "the chat never saw a token (its environment)" "a stored token is in the chat's environment"
  else step_pass "the chat never saw a token (its environment)"; fi
}
stored_tokens_match() { local in; in="$(cat)"; printf '%s' "$in" | grep -qF -f <(stored_tokens | grep -v '^$'); }

# ---- run ----------------------------------------------------------------------------------------------------

declare -a PROJECTS=() SIGNED=()
CLONE=""

printf 'one login, live: probes first, then the scenario. No token is printed or kept outside your keyring.\n\n== probes ==\n'
[ -n "$github_repo" ] && { probe github.com github && SIGNED+=(github.com); }
[ -n "$gitlab_repo" ] && { probe gitlab.com gitlab && SIGNED+=(gitlab.com); }
[ -n "$ghes_host" ] && { probe "$ghes_host" github && SIGNED+=("$ghes_host"); }
[ -n "$glhost" ] && { probe "$glhost" gitlab && SIGNED+=("$glhost"); }

if [ "$probes_only" = 0 ]; then
  [ -n "$github_repo" ] && scenario github.com github "$github_repo"
  [ -n "$gitlab_repo" ] && scenario gitlab.com gitlab "$gitlab_repo"
  [ -n "$ghes_host" ] && scenario "$ghes_host" github "$ghes_repo"
  [ -n "$glhost" ] && scenario "$glhost" gitlab "$glhost_repo"
  printf '\n== the chat saw no token ==\n'
  chat_saw_no_token
fi

printf '\n%d passed, %d failed, %d skipped.\n' "$pass" "$fail" "$skip"
if [ "${#PROJECTS[@]}" -gt 0 ]; then
  printf '\nTo clean up (a chat may not, by design), close these and delete their branches yourself:\n'
  for p in "${PROJECTS[@]}"; do printf '  %s  (branch %s)\n' "${p% *}" "${p##* }"; done
fi
[ "$skip" -gt 0 ] && printf 'Skipped steps are NOT proven: each names the slice it waits for.\n'
[ "$fail" -eq 0 ]
