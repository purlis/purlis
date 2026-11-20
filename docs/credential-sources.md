# Where purlis's forge credential comes from

One login (ADR 0128, proposed in #7269): purlis holds the person's forge credential and acts with it for a chat; a chat
never holds the token. The forge proxy asks one question of one trait,
`CredentialSource::credential_for(host)` (`crates/purlis-core/src/forge/credential.rs`), and two
sources answer it.

## The two sources

| | Interactive sign-in | Token connection |
|---|---|---|
| For | A person in the window | A CI job, `purlis serve`, a pool runner, the operator's Secret |
| Where it is | The keyring (`purlis/@forge/<host>/<id>`, ADR 0077) | The process's environment, or a file a variable names |
| github.com | Device flow, client id only | `GH_TOKEN`, `GITHUB_TOKEN` |
| GitHub Enterprise Server | Device flow, through the application an owner registered on that host and the person pinned (`forge-registrations.json`) | `GH_ENTERPRISE_TOKEN` with `GH_HOST`; `GITHUB_TOKEN` with `GITHUB_SERVER_URL` |
| gitlab.com | PKCE, 2 h access token and a rotating refresh token | `GITLAB_TOKEN` with `GITLAB_HOST`; `CI_JOB_TOKEN` on gitlab.com's own CI |
| Self-hosted GitLab | PKCE, through the OAuth application a group owner or an administrator registered on that host and the person pinned | `GITLAB_TOKEN` with `GITLAB_HOST`; `CI_JOB_TOKEN` with `CI_SERVER_HOST` |
| Fallback | A pasted personal access token | Any host: `PURLIS_FORGE_TOKEN_<HOST>` |
| Renewal | Before it expires, without the person (`forge::credential::renewal`) | None for a per-job token; an expiry warning for a long-lived one |

`<HEX>` is the lower-case hex of the host's bytes (ADR 0128 D9.2): `github.com` is
`6769746875622e636f6d`. One host has one spelling, and no two hosts share one. Per host, in order, the
first variable that is set decides: `PURLIS_FORGE_TOKEN__<HEX>`, `PURLIS_FORGE_TOKEN_FILE__<HEX>` (a
path: a Kubernetes or CI secret mounted as a file), then the forge CLI's own variables above.
Both of the first two set is a refusal, and one set and empty is an error, never a fallback to the
next. `PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>`, `PURLIS_FORGE_KIND__<HEX>` and `PURLIS_FORGE_ACCEPT__<HEX>`
(below) are the other names of a host; the window and `purlis doctor forge` show them, so nobody
computes one.

**A token is only offered to the host its variable names.**

| Variable | Offered to |
|---|---|
| `GITHUB_TOKEN`, `GH_TOKEN` | github.com, **only when `GITHUB_SERVER_URL` is unset or names github.com** |
| `GITHUB_TOKEN` | the host `GITHUB_SERVER_URL` names (Actions on GHES), never github.com |
| `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN` | the host `GH_HOST` names |
| `GITLAB_TOKEN` | the host `GITLAB_HOST` names, else the one `CI_SERVER_HOST` names; **no host named, no host** |
| `CI_JOB_TOKEN` | the host `CI_SERVER_HOST` names |

A variable that names a host that is not a host name names none, and nothing is offered on its
account.

## Token kinds, read from the token

Taken by the exact prefix and the shape that follows it (its characters and length), never by how it
begins. ADR 0128 D9.

| Token | From `GH_TOKEN`, `GITLAB_TOKEN` and the other CLI variables | From `PURLIS_FORGE_TOKEN__<HEX>` | Lifetime |
|---|---|---|---|
| `ghs_…` (an installation token; Actions' `GITHUB_TOKEN`) | taken | taken | Per job: never renewed, never warned about |
| `github_pat_…` fine-grained | taken | taken | Long-lived |
| `ghu_…`, `gho_…` | **refused** | taken | Per job (eight hours, and no refresh token here) |
| `ghp_…` classic | **refused** | **refused** | A classic token reaches every repository its owner can; use a fine-grained one |
| `ghr_…` refresh token | **refused** | **refused** | Not a credential for the API |
| `CI_JOB_TOKEN`, `glcbt-…` | taken | taken | Per job |
| `glpat-…` | taken | taken | Long-lived. A prefix cannot tell a personal, project and group access token apart |
| anything else | **refused** | refused unless `PURLIS_FORGE_ACCEPT__<HEX>` is `unrecognised` | Long-lived |

A host that is not github.com or gitlab.com, with a token of no known shape, needs
`PURLIS_FORGE_KIND__<HEX>` (`github` or `gitlab`). Where the variable, the hint, the shape and the
host name different forges the token is refused, never guessed at.

A long-lived token's expiry is not in the token. Give it as `PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>`
(unix seconds, or like `2027-03-01T00:00:00Z`): within 14 days of it there is a warning
(`Expiry::Soon`), and after it there is no credential.

## The rule when both exist

**The mode picks the source, and it picks one.**

- **Interactive (the window):** the signed-in account only. A token in the environment is not read:
  a `GITHUB_TOKEN` exported in the shell the window was started from is nobody's choice of identity.
- **Headless (CI, `purlis serve`, a runner):** a token connection **for that host** wins. Only when
  none is given for it does the machine's signed-in account answer, if there is one.
- **No fallback after a failure.** A token that is given and unusable (not a token, expired, of the
  wrong forge) is the answer, and the sign-in is not tried in its place. A sign-in the forge ended is
  the answer, and no token connection stands in. Nothing falls back to a forge CLI's login (ADR 0070 §5).

## Renewal and the Notice

`Renewal::sweep` renews every signed-in grant with less than 15 minutes left (a request renews at
5), through the same refresher and the same locks as a request: one refresh at a time in a
process, and one at a time on the machine (`forge-refresh.lock`), so the window and a session host
never spend the same single-use refresh token. The rotated pair is written to the keyring **before**
the new token is used; if the keyring refuses, both pairs are held in memory, the write is tried
again first, and the spent refresh token is never sent. A renewal that cannot take the machine lock
is not made (it fails closed: the token in hand serves while it lasts). A pair the keyring refused
also leaves a marker in the lock folder (`forge-refresh-held-<hex>`: the account id and the process
id, written under the lock), so another process does not send the refresh token the forge has spent;
and a pair is written only over the refresh token it was made from, so a stale one never overwrites a
fresh sign-in or makes an item again that another process ended.

**Only the forge's word that the grant is over ends a sign-in**: `invalid_grant`, and GitHub's
`bad_refresh_token`. The account is marked signed out, its keyring item is deleted, and the window
is told once, which draws "Sign in again to `<host>`". Anything else (`invalid_client`,
`unauthorized_client`, a 5xx, a rate limit, a proxy, a network that is down) ends nothing: the
keyring item is kept, the token is used while it has time, the next try waits 30 seconds, then
longer, to 15 minutes, and after three failures in a row the window is told once (a Notice, "Sign in again").

**A renewal uses the client id the account signed in through**, never what the registrations say
now. If the host's pin is another application, or none, renewal stops, the item is kept, and the
window is told once, and told again if the pin moves again after it was put right; signing in again is
the person's press.

A token variable that is not text is an error, never unset; `GH_TOKEN` and `GITHUB_TOKEN` (and the
two enterprise variables) set to different tokens are refused, not ordered. `CI_JOB_TOKEN` is taken
whatever its length or characters (GitLab's job token is a JWT in newer versions). A chat is given
none of the connection variables, under `PURLIS_` or its old `CHARTER_` name.

## What is kept, and where

The token lives in the keyring or in the process's memory as a `SecretString`: zeroed when dropped,
kept out of the log from the moment it is read, in no `Debug` and no error. Nothing here writes one
to a file, an argument or an environment a chat reads.

## What `serve`, pool tools and the operator do today

- **`purlis serve`** touches no forge credential. A headless `serve` reads one through `Credentials`
  in headless mode.
- **Pool tools** (`app/src-tauri/src/pooltools.rs`) hand a runner chat's tool an installation token
  as `GH_TOKEN` (`forge::apptoken`). That is the per-agent token ADR 0128 replaces, so it is left
  for the proxy to remove rather than routed through here.
- **The operator** (`operator/`) is a workspace of its own that links nothing of `purlis-core`. It
  mints an installation token from the organisation's App Secret for a vouched runner and holds no
  person's credential. What it mints is a `ghs_` token, which a token connection reads as per-job.
