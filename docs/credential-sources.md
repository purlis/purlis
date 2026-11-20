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
| GitHub Enterprise Server | Device flow, through the application registered on that host (`forge-registrations.json`) | `GH_ENTERPRISE_TOKEN` with `GH_HOST`; `GITHUB_TOKEN` with `GITHUB_SERVER_URL` |
| gitlab.com | PKCE, 2 h access token and a rotating refresh token | `GITLAB_TOKEN` |
| Self-hosted GitLab | PKCE, through a user-owned OAuth application on that host (no administrator) | `GITLAB_TOKEN` with `GITLAB_HOST`; `CI_JOB_TOKEN` with `CI_SERVER_HOST` |
| Fallback | A pasted personal access token | Any host: `PURLIS_FORGE_TOKEN_<HOST>` |
| Renewal | Before it expires, without the person (`forge::credential::renewal`) | None for a per-job token; an expiry warning for a long-lived one |

`<HOST>` is the host upper-cased with `.` as `_`, `-` as `__` and a port's `:` as `___`:
`ghe.example.com` is `GHE_EXAMPLE_COM`, `git-example.com` is `GIT__EXAMPLE_COM`. Two hosts never
share a name. Per host, in order, the first variable that is set decides: `PURLIS_FORGE_TOKEN_<HOST>`,
`PURLIS_FORGE_TOKEN_<HOST>_FILE` (a path: a Kubernetes or CI secret mounted as a file), then the
forge CLI's own variables above. **A token is only offered to the host its variable names**:
`GITHUB_TOKEN` answers github.com, never `ghe.example.com`.

## Token kinds, read from the token

| Token | Kind | Lifetime |
|---|---|---|
| `ghs_…` (an installation token; Actions' `GITHUB_TOKEN`) | installation | Per job: never renewed, never warned about |
| `ghu_…`, `gho_…` | user-to-server | Per job (eight hours, and no refresh token here) |
| `ghp_…`, `github_pat_…` | classic, fine-grained | Long-lived |
| `CI_JOB_TOKEN`, `glcbt-…` | GitLab job | Per job |
| `glpat-…` | GitLab personal, project or group access token | Long-lived. A prefix cannot tell the three apart |
| anything else | unrecognised | Long-lived |

A host that is not github.com or gitlab.com, with a token of no known shape, needs
`PURLIS_FORGE_KIND_<HOST>` (`github` or `gitlab`). Where the variable, the hint, the shape and the
host name different forges the token is refused, never guessed at.

A long-lived token's expiry is not in the token. Give it as `PURLIS_FORGE_TOKEN_<HOST>_EXPIRES_AT`
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
5), through the same refresher and the same lock as a request, so one process rotates a single-use
refresh token. A forge that **refuses** ends the sign-in: the account is marked signed out, its
keyring item is deleted, and the window is told once, which draws "Sign in again to `<host>`". A
forge that cannot be reached ends nothing; the token is kept while it has time. `Renewal::keep` runs
the sweep on its own thread.

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
