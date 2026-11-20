# Your sign-in, brokered: one approach for every forge

**Proposed 2026-11-20, round 5.** Round 1 (numbered 0127 there; 0127 is the project-policy ADR on
`m75-policy`) proposed a per-owner App and GitLab project tokens, and four reviewers rejected it.
Round 2 proposed per-user tokens as the base and a bot as an opt-in second mode; the person
rejected two modes. Round 3 was one approach. Three reviewers (security, feasibility, coverage)
read round 3 and said **fixes needed, the shape stands**; round 4 folded the steward's rulings on
their findings. Two reviewers read round 4 (Fable round 3) and said **fixes needed, the shape
stands**; **this round folds the steward's fifteen rulings on those findings**. Where a ruling made
a claim the record had to check, what was checked and what was only read from documentation is said
where the claim is made. The facts and their sources are in
[`docs/research/one-login-forge-identity.md`](../research/one-login-forge-identity.md), sections 7
and 8. Nothing here is built and nothing was run against a live forge. What was read and what was
tried is said where each claim is made. **Slices A (proxy core), C (credential sources), D
(readiness) and E (the `gh`/`glab`/`git` spike) build against this record; its section numbers
(D1 to D10) are the ones they were given.**

## The requirement

Someone has a personal repo, an organization repo, a repo on GitHub Enterprise Server (GHES), on
gitlab.com or on a self-hosted GitLab. **Anyone with access to it** signs in to their own forge
account **once per forge host**, clones the repo as a project and works. The chat runs `gh`, `glab`
and native `git fetch`, `pull` and `push`, opens pull and merge requests, comments and reads CI.
The person types no command and no token, does no setup, and **is never asked to sign in again
because a token ran out**. One approach covers all of it, and it **replaces** Agent identity (the
bot App), the GitLab vault tokens (#7230) and the brokered push's use of the CLI login.

## The decision, in one paragraph

**The person signs in once per forge host with purlis's own sign-in (ADR 0077). purlis keeps that
credential in the keyring, in the app only. A chat holds no token. A forge proxy, a real HTTP
reverse proxy that terminates in the app, acts as the person for the chat's own recorded repos
only, on one default-deny allowlist (D3).** Merges, releases and pushes that touch CI files are
not refused dead: the chat asks, the person approves in Asks, and the app performs the act with the
sign-in (D8). **The same approach holds for every chat, locked or not, at every floor: a chat
never uses the person's own `gh` or git login, and the egress proxy denies the forge hosts
directly (D3g).** There is no bot, no installation, no project token, no `gh` login import and no
second mode. Where no person is present (CI, `purlis serve`, the operator), a **token connection**
supplies the credential and the proxy and its refusals are the same (D9).

## D1. The sign-in is an OAuth app, on every host

### D1.1 Why an OAuth app on GitHub and not a GitHub App

The sign-in is the one purlis already has: an OAuth app with the device flow, a public client id
and no secret (`forge/registration.rs`, `forge/device.rs`). It grows scopes; it gets no sibling.
The alternative, a GitHub App's user token, was compared case by case (research 7.1):

| Whose repo | OAuth app | GitHub App user token |
|---|---|---|
| The person's own personal repo | works | the App must be installed on their account (their own press) |
| **Someone else's personal repo**, the person a collaborator | **works** | **cannot work**: only the owner can install on a personal account |
| An organization with no OAuth app restrictions | **works, nothing for anyone to do**: "the application will automatically be authorized" | an owner (or a repo admin) must install it |
| An organization **with** OAuth app access restrictions | the member **requests**, an owner approves once | the member **requests an install**, an owner approves once |
| An outside collaborator in a restricted organization | requests only if the owner allows requests (D10 has the guide) | the same limit |

The OAuth app is **at least as wide in every row, and strictly wider in two**. A GitHub App is the
better tool for a bot, and there is no bot here. What the OAuth app costs: its token is scoped by
`repo`, not by repository, and "you can't scope source code access to read-only" (GitHub,
*Authorizing OAuth apps*). That is the proxy's job to narrow (D3), as it would have been for any
App user token.

**The registration is the existing one**, `forge/registration.rs`, with scopes **`repo` and
`read:org`**, and nothing else at sign-in. Not `delete_repo`, not `admin:*`, not `write:packages`,
**and not `offline_access`**: that is a GitHub App user-token request, which an OAuth app does not
make (D2 says what an OAuth app's expiry is). **`workflow` is not asked at sign-in**: GitHub then
refuses a push that adds or changes a workflow file, so the CI-file protection holds on GitHub even
if the proxy has a bug. **The everyday token never has `workflow`.** A held CI push (D8.3) uses a
**second token**, minted by a second sign-in on that press with `workflow` asked, kept as its own
keyring item, used **only by the app for held acts** and never injected on a chat's route (D3).
Whether GitHub keeps the two tokens' scopes apart (it may fold a re-consent into the existing
grant) is a gating probe (Open item 3); **if it does not, a CI-file push on GitHub is not
offered and the ask says to push it from the person's own terminal**, rather than the everyday
token gaining `workflow`.

**Other facts this record relies on (read from GitHub's documentation, not run).** GitHub
**revokes an OAuth app's token that has gone unused for one year** (the same sentence as a
revocation in D2). **A fine-grained PAT and an Actions `GITHUB_TOKEN` return no `X-OAuth-Scopes`
header**, so a check of a token connection's reach is made **by a probing call**, never by that
header (D10.1).

### D1.2 The one case an owner decides, stated exactly

**An organization that has enabled OAuth app access restrictions** (on by default when an
organization is created: GitHub, *About OAuth app access restrictions*) **refuses an unapproved
OAuth app's reach into the organization's private resources until an owner approves it.** Nothing
purlis does removes this, and no design on GitHub removes it, for an App or an OAuth app. What
happens:

1. At sign-in, GitHub's authorization page lists the person's organizations and, for a restricted
   one, offers the person **a request to the owners**. It is GitHub's page and the person's press.
2. Until an owner approves, the app reaches only the organization's **public** resources. At
   clone, and in the readiness view (D10), purlis asks the forge as the person, recognises the
   refusal and shows one Notice: *"acme has not approved purlis. Ask an owner to approve it."* with
   **the owner guide and a request text to copy** (D10.2). Public repos, and the person's own and
   other organizations' repos, work meanwhile.
3. The owner approves **once per organization, for everyone**. Then every member, and every outside
   collaborator the owner lets request, works with no further act. The existing token gains the
   organization's access (expected from how approval is described; confirmed in the first live
   probe, Open item 3).

**The `gh` login import is dropped.** Round 3 offered the person GitHub CLI's status as a
*privileged OAuth app* as a way past the owner. That is withdrawn: a chat and the app never use
another vendor's client id, a login minted for `gh` is a second credential with a second
revocation path, and "import the CLI's login" is the opposite of **one approach**. **An
OAuth-restricted organization gets the readiness guide to the owner's one approval, not an
import.** ADR 0077 amended 2026-10-12-04 (the import of an existing CLI login) is **superseded
for GitHub** by this record. A person who still has a login of the CLI's keeps it for their own
terminal, which purlis does not touch.

Two more cases that look like this one and are not (each has its guide in D10.2):

- **SAML single sign-on organizations.** The person must have an active SAML session when
  authorizing, and after an owner enforces SSO "you must reauthorize any OAuth apps". This is the
  person's own browser step (GitHub, *About authentication with SAML single sign-on*), and it is
  the one place the person **signs in again for a reason that is not the token's lifetime**.
- **An account GitHub has flagged** cannot authorize any third-party app. Not purlis's to fix.

### D1.3 The other hosts, and who registers

| Host | Sign-in | Registered by | Notes |
|---|---|---|---|
| github.com | The existing OAuth app, device flow | purlis (built) | scopes `repo`, `read:org`; one re-consent for existing sign-ins that lack `read:org` |
| gitlab.com | The existing OAuth application, PKCE | purlis (built) | scope **`api` only**, as `forge/registration.rs` asks (its test pins `["api"]`). Round 4 wrote `api`, `write_repository`, `read_user`; **that was not what is built and is withdrawn.** `api` is the broader scope and covers the API and git over HTTP; nothing narrower is added. **What changes:** nothing in the registration, and a GitLab sign-in already made stays valid. `api` cannot be narrowed to a project, so narrowing is the proxy's list, as on GitHub (D3). A narrower grant (`read_repository`, `write_repository`) would not reach merge-request APIs, so it cannot be the base |
| **GHES** | An OAuth app, device flow (*Enable Device Flow* ticked) | **an owner**: owned by an **organization** (or enterprise) the owner administers, never by an individual user's account | per-host registration store (D2) |
| **Self-hosted GitLab** | An OAuth application, the device flow (17.1 and later; generally available 17.9), else PKCE | **an owner of a group** (group-owned) or an **administrator** (instance-wide), never a single user's own application | per-host registration store (D2) |

**Round 3 let any user of the host register the application. That is withdrawn.** A user-owned
application is owned by one person, can be deleted or re-pointed by them, and is a lookalike
waiting to be shared: the first person who registers decides what every teammate signs in to. The
registration is an **organization- or group-owned** application, made by an owner, whose owner
and name are on the forge's own page. Three rules make it safe:

1. **The registrant is shown at sign-in, on the forge's own consent page.** **No API returns an
   OAuth application's owner to purlis** (the device-flow and token replies carry none), so purlis
   cannot print it from a response and must not pretend to. The owner is the line GitHub's and
   GitLab's **authorization page** shows (*"owned by acme-platform"*), and the window says before
   the press: *"On the next page, check that the application is owned by acme-platform on
   ghe.example.com. Sign in only if it is."* The expected owner is the **pinned** one (rule 2);
   the person compares. **Whether the consent page shows the owner on a GHES version and on a
   self-hosted GitLab is a gating probe (Open item 3), and a host where it does not is a token
   connection (D9)**, not a sign-in the person cannot check.
2. **The client id is pinned per machine, on the person's press.** A client id arrives from a
   teammate (the workspace may *suggest* one, a value shown for the person to confirm) and takes
   effect only when the person presses *Use this application* for that host. The pinned id is a
   **Machine, device-bound** item, never synced as an instruction: a sync that changed it would be
   an attacker's way to point every teammate's sign-in at their own application. A changed id is a
   new press and a Notice.
3. **Where the person cannot register one, an owner does.** The window shows the values to enter
   (name, scopes, redirect for PKCE) as a request to copy, as in D10.2. A GitLab administrator who
   turned off user-owned applications (the `user_oauth_applications` setting) does not block this:
   group-owned and instance-wide applications are unaffected. Where nobody will, the host gets a
   **token connection** (D9), which has no automatic renewal.

Where GitLab 19.6 or later ships its own `glab` application on a self-managed instance, it is
**glab's** application, not purlis's, and purlis does not use its client id.

## D2. The credential's lifetime: when it ends, and when the person signs in again

The **app alone** holds and refreshes the credential (ADR 0077 §3: one process may rotate a
single-use refresh token), before expiry and once on a 401. A closed app misses nothing that
refresh covers.

**GitHub, an OAuth app.** Round 3 said the sign-in asks for `offline_access` and gets 8-hour
tokens. **That is wrong for an OAuth app and is withdrawn.** The check this round made, **from
GitHub's documentation as known and not run live**: *"Expire user access tokens"* (the setting, the
`expires_in`, the `refresh_token` and the six-month refresh token) **is a GitHub App's optional
feature. It is not an OAuth app's.** Round 4 wrote that an OAuth app's owner can turn it on; **that
is not supported by what was read and is withdrawn.** So, for an OAuth app, which is purlis's
registration on github.com and the shape of a GHES one (D1.1):

- **The access token does not expire, and there is no refresh token.** There is nothing for the app
  to renew. **The credential ends only by revocation** (the person, an owner or the forge), by
  membership ending, by SAML reauthorization (D1.2), **and by GitHub's own rule that an OAuth
  app's token unused for one year is revoked**. A person who does not open purlis for a year signs
  in again, once: that is a limit of GitHub's, shown as a Notice with the fix, never as a failed
  push.
- **A non-expiring token is a longer-lived secret.** D6 says so. The keyring and the app-only rule
  protect it, and purlis does not claim a lifetime it does not control.
- **Nothing in this record depends on expiry or refresh.** The sign-in still reads `expires_in` and
  `refresh_token` from GitHub's reply and refreshes if they are present (a GHES release or a
  changed registration could return them), but no path assumes it. **The probe (Open item 3) reads
  the live reply once** and records what came back. **This is kept out of the ADR 0070 §4
  amendment** below, which says only that a GitHub token's lifetime is the registration's and the
  host's, and that GitLab's expires.

| Host | Access token | Refresh |
|---|---|---|
| github.com | **does not expire** (OAuth app); revoked after a year unused | none, unless a reply carries one (read, never assumed) |
| GHES | per **that host's registration** (below); read from the reply the same way | the same |
| gitlab.com | 2 hours (7200 s) | refresh token "can be used even after the access token itself expires"; each refresh replaces both; PKCE, no secret |
| Self-hosted GitLab | 2 hours by default (an administrator may change it, 19.1 and later) | the same |
| Token connection | the token's own expiry, or none | **none** (D9) |

**GHES gets a per-host registration store.** Each GHES host has its own registration (client id,
whether the app expires tokens, the pinned owner, D1.3) in a store of its own, keyed by host. The
github.com registration is never reused for a GHES host and the reverse. The store names its tier
in `docs/plane-format.md`: **Machine, device-bound** (nothing secret: a client id, a flag, the
owner's name), rebuildable by signing in again.

**Several accounts on one host.** A person may have a personal account and an enterprise-managed
(EMU) one on the same host. **A credential is keyed by host and account** (`purlis/@forge/<host>/
<account>`), never by host alone, and **which account serves a project is chosen per project** by
the person's press when the repo is cloned or trusted (D4 records it). A chat uses only its
project's account. One account per host is the default and asks nothing.

**The person signs in again only when**: (1) a token goes unused past its host's limit (GitHub:
one year for an OAuth app's token; a refresh token where the host expires them: GitLab's, a limit
of the forge's and not a purlis choice);
(2) they or an owner **revoke** the authorization, or a GitLab administrator invalidates it; (3) a
**SAML** organization needs reauthorization (D1.2); (4) a **token connection** reaches its expiry
(D9); (5) the forge account is changed or flagged. Every one shows as a Notice with the fix in one
press (D6 of ADR 0077's pattern, D5 here), never as a failed push.

## D3. The forge proxy

A chat holds **no token**. Three tools reach the forge for it, and all three reach one proxy.

### D3a. What it is

**A real HTTP reverse proxy that terminates in the app**, where the keyring is. It is **a route
inside each chat's own per-chat HTTP egress port** (ADR 0067 amended 2026-10-10-01: the port is the
chat), not a second port and not a TLS interceptor. It speaks plain HTTP on that loopback port to
the chat and TLS to the forge, with the system's roots. **An egress proxy held elsewhere** (a session host's,
a runner's) does not hold the credential: it **forwards the forge calls to the app's proxy**, and
the app answers them. So a chat in a container, over SSH or on a runner reaches the same proxy
with the same rules (D9 says what a runner with no desktop does).

**ADR 0067's amendments this extends, each named** (their text is changed by D3b and the
amendment list below, never edited in place):

- **2026-10-10-01 item 1** (one pair of ports per chat) **stands and is what this is built on.**
- **Item 2** (*"nothing a connection sends names a chat"*): the port still is the chat; for the
  forge route **the port is joined by a per-chat secret** (D3b). Nothing else a connection sends
  is read as a chat.
- **Item 4** (the local-address check): unchanged. The forge route is **served by the proxy
  itself**, which dials the forge's public address; a chat's request to its own port is not a
  connection to "this machine" in the sense the check refuses, and the exception is **that exact
  port, for that chat**, listed.
- **Item 5** (*"host and port only … no path rules"*): **amended for the forge route only.** A
  tunnel (`CONNECT`) is still host and port only. A plain-HTTP **origin-form** request on the
  chat's own port, for the forge route, is read: method, canonical path, headers and, for GraphQL
  and a push, the body.
- **Item 7** (*"no buffer past a fixed size"*): the forge route **streams** (clone and fetch packs,
  release assets, artifacts) with a bounded window; **the one exception is a push whose changed
  paths are read (D8.3)**, which is held up to a size limit (Open item 5).
- **2026-10-10-02 item 4** (*no other local port*; `allowLocalBinding` false): unchanged, and
  D3h.2 says what a managed setting that turns it on costs.

**Reused (built):** ADR 0067's per-chat egress proxy and its sandbox profile (amended 2026-10-10,
#1664), which allows a chat's network traffic only to its own proxy; `forge/refresh.rs`,
`device.rs`, `pkce.rs`; `mergehold.rs`, `diffscan.rs`; the keyring items of ADR 0077 §7 and the
recorded clone list of ADR 0077 amended 2026-10-29 (E3). **Not built:** the key proxy of #6762 and
#6812 is specified and has no code in `crates/` at `4568a3c7b4`. **This record's proxy is new
code**: the reverse proxy, the credential injection (it removes every credential header the chat
sent and adds the person's), the canonical-path allowlist (D3d), the GraphQL resolver (D3e), the
git-protocol checks (D3f), the response rewrite and the audit (chat id, repo, method, canonical
path, verdict).

### D3b. The chat's own port, and who is asking

**The forge route lives on the chat's own per-chat HTTP egress port** (ADR 0067 amended
2026-10-10-01 item 1: one pair of ports per chat, opened when it starts and closed when it ends).
Round 4's "one port for every chat on the machine" is **withdrawn**: ADR 0067's rule is that the
port is the chat, and a shared port would have needed the chat to say who it is. The route
extends that port to **origin-form** requests (a request line `GET /acme/app.git/info/refs`, not
only `CONNECT` and absolute-form) and to **streaming** (D3a).

**Identity is that port plus a per-chat secret.** The port names the chat; the secret is the
second factor that makes a process that merely reaches the port (the harness, a hook: D3h.1) not
enough:

- **The secret is at least 128 bits from the operating system's random source**, minted **at every
  chat start and at a resume** and never reused, held in the app's memory and the chat's
  environment only, and worth nothing anywhere else.
- **It is compared in constant time.** **Attempts are limited**: a port that receives more than a
  few wrong secrets in a minute answers nothing for the next minute and raises one Notice naming
  the chat. A wrong secret is refused as IN_DOUBT and audited.
- **Each tool carries it where it already sends a credential**, and the proxy reads it from only
  these places, **strips them all** and injects the person's (D3a): git, in a
  `Proxy-Authorization: Basic` header set by `http.extraHeader`; `gh`, as its token
  (`Authorization: token …`, from `GH_TOKEN`); **`glab`, as its token (`PRIVATE-TOKEN` or
  `Authorization: Bearer …`, from `GITLAB_TOKEN`)**. Round 4 said the host and secret travel as
  `Proxy-Authorization` for all three and that `GITLAB_TOKEN` was that secret; **`glab` does not
  send `GITLAB_TOKEN` as `Proxy-Authorization`, so that did not work and is corrected here**: the
  secret is the same value, in the header each tool really sends, and **the host is not in the
  credential**: it is the next bullet.
- **The host is named by the request and checked, never trusted.** The chat's URL begins with the
  forge host's name as its first path segment (git, `glab`) or is the virtual host `gh` is given
  (D3c). The proxy requires that name to be **one of the host names this chat's recorded repos
  are on** (D4), and **builds the upstream from its own recorded value, not from the request's
  text**: a request that names another host, or whose `Host`, `X-Forwarded-Host`, `Forwarded`,
  `X-Original-URL` or similar header disagrees, is refused. **So no path segment chooses an
  upstream**: it selects among the hosts the chat already has, and the upstream is the proxy's own
  string. (The unrecorded-repo read route has a fixed list of hosts and no credential: D3i.)

Why not a kernel read of the connecting process (a TCP loopback socket has no peer credential; the
unix-socket forms are not spoken by `git` and `glab`), is round 3's weighing, unchanged (research
7.3).

**The one exception: a chat the person chose to run with no sandbox** (ADR 0124's No sandbox; the
person decided to run it unconfined). Such a chat **gets the same proxy route and the same
environment**, so forge works in it exactly as in any chat. What purlis **cannot** do there is stop
the chat's own use of the person's login: it can read `~/.config/gh`, the keychain and the git
credential helper as the person, and there is no egress rule to deny the forge hosts (D3g).
**D6 and the point-by-point table say this plainly**: at No sandbox the proxy is the route purlis
provides and not a guard, and the hold on merges and tags is, as ADR 0124 §7.3 says, to catch
accidents only. This replaces round 4's "an unsandboxed chat gets no proxy route".

### D3c. How each tool is pointed at it

A chat's git configuration is passed in the environment, as #7241 does, and the clone's
`.git/config` is not touched. Every route is **to the chat's own port** (D3b).

- **git.** **One `url.<proxy>/<host>/<owner>/<repo>.git.insteadOf` per recorded repo**, plus one
  broader `url.<proxy>/anon/<host>/.insteadOf https://<host>/` for the **anonymous read route**
  (D3i), so an unrecorded public repo's dependency fetch has a way and **no credential**. git
  takes the **longest matching prefix**, so a recorded repo always takes the credentialed route
  and every other path on a forge host takes the anonymous one. Requests are **origin-form** to
  the chat's port with the secret in `http.extraHeader`. **No credential helper at all**: the list
  stays empty as #7241 makes it, so `git credential fill` yields nothing. **Reconciling the
  rewrite with "no path segment names an upstream" (D3b):** the first segment `<host>` is a
  **lookup key into the chat's recorded hosts**, and the upstream string is the proxy's own;
  `/anon/<host>/` is a lookup into a **closed list** (the forge hosts the app knows and the
  storage hosts of D3i), never an open host.
- **glab.** `GITLAB_HOST` is the real host name, `GITLAB_API_HOST` is the loopback address and
  port, `GLAB_API_PROTOCOL=http`, `GITLAB_TOKEN` is the chat's secret (a value glab will not run
  without; it is sent as `PRIVATE-TOKEN` or a bearer and the proxy accepts it there, D3b).
  Origin-form requests to the port, like git.
- **gh.** **Through a wrapper purlis writes for each chat that clears `NO_PROXY` and `no_proxy`**
  and sets `HTTP_PROXY` to the chat's port, and sets `GH_HOST=github.localhost` (`GH_TOKEN` the
  chat's secret, sent as `Authorization: token …`, D3b). Go never proxies `localhost` and loopback
  hosts while `NO_PROXY` can name them, which would send the call straight to nowhere; the wrapper
  removes that case. The wrapper picks the host from the working directory. For GHES, the same
  virtual host maps to `https://<host>/api/v3/` and `/api/graphql`: `gh` has no plain-HTTP mode for
  an enterprise host, and this gives it one.
- **The proxy re-emits canonical URLs.** It never forwards the chat's own request line: it parses,
  validates against D3d and builds the upstream URL itself. `Link` response headers are rewritten
  to the chat-side address, or `--paginate` would leave the proxy.
- **Pinned versions.** purlis names the **proven `gh` and `glab` versions** (the spike's, E:
  `gh` 2.69.0 and `glab` 1.119.0 are what was read) and a chat whose `gh` is another version gets
  the wrapper's one sentence naming the proven one. An unproven version is not blocked from
  `git`, only from the `gh` route.

**Not proven.** git's proxying of a `*.localhost` name, that a live `gh` or `glab` completes
`pr create`, `pr checks`, `run view`, a paginated list or a GraphQL query through the proxy, **and
that `glab` sends `GITLAB_TOKEN` in the header D3b says**, were **not run** (the sandbox of the
session that wrote this record refused every loopback bind and connect, and it was not worked
around). That run is **slice E's** and gates the build. **Fallback** for git, with no dependence on
proxy variables: rewrite straight to `http://127.0.0.1:<port>/<host>/<owner>/<repo>.git`. If `gh`
fails the run, **the gap is named as `gh`'s, and `git` and `glab` still ship**; no purlis CA, no
shim that reimplements the API and no token in an environment is acceptable.

### D3d. The REST allowlist: default-deny, by (method, canonical path)

Everything is refused unless a rule below allows it. A request is matched on **method and the
canonical path**, after: percent-decoding **once** (a second decode is a refusal, so `%252e` is
refused as double encoding), refusal of `.`, `..`, `//`, backslash, NUL and control characters,
case folded where the forge folds it, and no prefix matching (`acme/app` is not `acme/app-secrets`).

**Under the recorded repo** (matched on **owner/name and numeric id**; D4), the allowed
routes are an explicit list, not "everything below `/repos/o/r`":

- git smart-HTTP: `info/refs`, `git-upload-pack`, `git-receive-pack` (D3f);
- refs and commits, read: `git/refs`, `commits`, `compare`, `contents` (read), `branches` (read);
- branches, write: create a ref under a name the chat created (D3f), nothing else;
- pull requests: list, get, create, update, comment, review comment, **review with COMMENT or
  REQUEST_CHANGES** (not APPROVE: D3e), files, commits, requested reviewers;
- issues, labels, assignees, milestones (read), comments;
- CI read: `actions/runs`, `jobs`, `logs`, `artifacts` (list; **download**, the forge's redirect
  to a storage host, followed once and anonymously: D3g), `check-runs`, `statuses`;
- **releases and assets, read:** the release list and `assets/:id` downloads, the same redirect
  rule (D3g); a release's **creation** is a held act (D8.2);
- **forking is not on this list.** A fork is a held act (D7, D8.6), on the person's press.

GitLab has the same shape under `/api/v4/projects/:id`, where **`:id` is the recorded path or the
recorded numeric id**, nothing else.

**A named non-repo read list**, and only that: `GET /user`, `GET /rate_limit`, `GET /meta`,
`GET /user/orgs` and `/user/memberships/orgs` (for readiness), `GET /orgs/:org` for an org the
person has a recorded repo in, `GET /api/v4/user`, `GET /api/v4/version`, and the equivalent
discovery endpoints. Each is a read.

**Refused, whatever the token could do:**

- `/repositories/:id` (the numeric alias) **unless that id is recorded**; likewise
  `/projects/:numeric` on GitLab unless recorded: an alias is a second spelling of a repo, and a
  recorded id is the check that it is the same repo;
- **`/search`** in every form (code, issues, repositories, users): it reads across every repo the
  person can see;
- **`POST /user/repos`, `POST /user/keys`**, and every other `/user/*` write, `/orgs/*` write,
  `/enterprises/*`, `/admin/*`, `/applications/*`, `/authorizations`, and any other host's path;
- **the sign-in pages** (`/login/device`, `/login/oauth/*` and GitLab's equivalents: D3j);
- **forks from a chat at all** (D7: the person's press makes one), and in particular **forks to
  another org** (a fork request that names `organization`, or a GitLab fork with
  `namespace` other than the person's own);
- **`target_project_id`** and any pull request or merge request whose base repo is not the
  recorded one (a PR from the fork into the upstream is allowed only where the upstream is itself
  recorded);
- **writes to the default branch** (D8 holds the merges; a direct write is refused), and a push,
  create or delete of a protected branch, and `POST /repos/:o/:r/merges` (the merge-branch
  endpoint: it merges without a PR);
- repository, hook, secret, deploy-key, ruleset, branch-protection and settings writes.

### D3e. GraphQL, and the other ways to merge

**GraphQL** is parsed, never forwarded as text. The proxy resolves **after variables, aliases and
fragments are expanded**, and then:

- every **root field** must be on the allowlist (a short list of reads and of the mutations that
  mirror D3d's allowed REST routes);
- **every ID that appears is resolved to a repository** (a node ID's type and parent, queried by
  the proxy as the person) and that repository must be a **recorded** one; an ID that cannot be
  resolved is refused;
- **`node`, `nodes` and `search`** are refused whole (they read any object by ID or any query),
  and **every mutation not on the list is refused**, including every merge and auto-merge
  mutation (`mergePullRequest`, `enablePullRequestAutoMerge`, ... : D8);
- **every `owner`, `name`, `fullPath` and `nameWithOwner` argument is checked** against the
  recorded repos, as an ID is, wherever it appears (`repository(owner:, name:)`,
  `repositoryOwner`, GitLab's `project(fullPath:)`, `namespace`): a pair that is not a recorded
  repo is refused;
- **a nested connection that leaves the repo is refused**, on any root: `viewer.repositories`,
  `viewer.organizations`, `organization.repositories`, `repository.forks`, `repository.parent`,
  a pull request's `headRepository` and `baseRepository` where the other side is not recorded,
  `repository.owner.repositories`, and GitLab's `group.projects` and `user.projects`. The
  proxy walks the **resolved selection** and refuses the whole query where a field's type is a
  repository other than a recorded one; a selection it cannot resolve is refused;
- a batch with a refused part is refused whole.

**Merges by other means are all refused, and all of them go to D8 where an act exists:**

- **push options** (`git push -o merge_request.merge_when_pipeline_succeeds`,
  `-o merge_request.target=…`): the receive-pack request is read and a push with options is
  refused;
- **a review with `APPROVE`** (GitHub) and **GitLab's `/approve`** endpoint: an approval from
  the person is a step towards merge, so the chat does not approve its own work as the person;
- **bodies that carry a quick-action or bot command**: a comment, a PR or MR description or a
  review body whose line starts with `/merge`, `/approve`, `/rebase`, `/submit_review`,
  `/lock`, `@dependabot merge` or `@mergify`, `@kodiakhq`, `bors r+`, `/ship` or any line that a
  forge or a **known merge bot** reads as a command is **refused**, with the one sentence (the
  proxy reads these as request body data it already parses, not as a harness's output). **A
  label write** whose name is a known merge label is refused the same way. **The list is the
  known ones only**: a repository's own bot or a `labeled` / `issue_comment` workflow with a
  command nobody told the proxy is a named residual (D3h.7).

### D3f. git over HTTP

- **Push only to branches the chat created.** The proxy reads the ref commands that begin a push
  and allows only the ones that **create or fast-forward a branch the chat itself created** in
  this chat (the list is kept in the app, by chat, from the creating push). It refuses the default
  branch, a protected branch, a branch it did not create, **a tag (D8.2)**, a delete and a
  **non-fast-forward anywhere but the chat's own branch**. **`git push --force-with-lease` to
  the chat's own branch is allowed**: a chat that rebases its own work is not at a dead end (ADR
  0124 §16, §7.3: the lease to the chat's own branch is not held), and the ref command's old
  SHA must be the branch's current tip. A non-fast-forward to a branch the chat did not create
  is refused. A branch whose name matches a deploy trigger is refused (D3h.8). A person who wants the chat to continue on an existing branch
  of theirs names it in the window, which adds it to the list.
- **A push goes only to the address recorded for the clone**, passed to git explicitly, never to
  what `origin` resolves to now.
- **CI files** are held, not refused (D8.3); the proxy computes the tree diff of the push's new
  tip in a quarantine, which costs a buffered pack (Open item 5). **`diffscan.rs` is a pre-commit
  scanner that reads the index of a commit being made; reading a pushed pack and a tree diff
  against the clone's objects is new code** and needs the clone's object store (D8.3).
- **LFS, download only.** The `locks` API and a batch `upload` operation are not on the allowlist.
  A **download** batch (`/info/lfs/objects/batch` with `operation: download`) is served: for a
  recorded repo through the credentialed route, for an unrecorded public one through the
  anonymous route (D3i). The batch reply names **storage URLs on other hosts, with their own
  per-object credentials**; the proxy **rewrites each `href` to a proxy address, checks the
  host against D3i's closed storage list and fetches it itself**, so no LFS credential reaches the
  chat. An `href` on a host outside the list is refused and the window says *"This repo's LFS
  files live on a host purlis does not know."* Whether a live LFS clone completes through the
  route is a probe (Open item 1).
- **Submodules.** A submodule is another repo. It is **not recorded** unless the person trusts it
  as its own clone (D4), so it is not rewritten, its direct address is denied (D3g), and
  `git submodule update` fails with the one sentence naming the submodule. A person who trusts a
  submodule's repo gets it served like any other.

### D3g. Hardening, and the one approach for every chat

- **Redirects are never followed with the credential.** A 3xx from a **credentialed** route is
  not forwarded with the credential: a `Location` is a new host the credential is not bound to.
  Where the forge answers a release-asset or artifact download for a **recorded** repo by redirect
  to a storage host (a signed URL), the proxy may **follow it once, anonymously** (the credential
  dropped, the host on D3i's closed storage list, the signed URL being its own authority) and
  stream the answer. A redirect to any other host is returned as a refusal. Round 4 refused every
  such download; **that broke releases and is replaced** by this.
- **Override headers are stripped:** `X-HTTP-Method-Override`, `X-Method-Override`,
  `X-Forwarded-*`, `Forwarded`, `X-Original-URL`, `X-Rewrite-URL`, every `Authorization` and every
  credential-carrying header of the chat.
- **`Content-Length` and `Transfer-Encoding` together are rejected** (request smuggling), as is
  any duplicate of either that disagrees.
- **One request per connection**: the proxy answers with `Connection: close` and never reuses a
  connection between requests, so a second request cannot ride the first's verdict.
- **One approach for every chat, locked or not, at every floor** (the one exception, a person's
  own No-sandbox chat, is D3b's). A chat **never uses the person's own `gh` or git login**:
  `GH_CONFIG_DIR` is an empty folder, the credential helper list is empty, `GIT_CONFIG_*` is
  purlis's, at the watch floor and the full sandbox alike, and **the egress proxy denies the forge
  hosts directly** (`github.com`, `api.github.com`, `uploads.github.com`,
  `objects.githubusercontent.com`, `codeload.github.com`, the GHES host, `gitlab.com`, the
  self-hosted host), **so the only route to a forge is the one this section describes**, which is
  why D3i exists: a denied host with no anonymous way would break every dependency fetch. **This
  amends ADR 0067's amendment of 2026-11-19**: its *"under the shipped `watch` floor the person's
  `gh` and git over https keep working in a chat, on purpose"* (the keep) is withdrawn, and
  `forgelock::claimed` no longer decides whether a chat is on the lock: every chat is. The class-6
  credential files are denied at the full sandbox as before.

### D3h. The residuals, named

What this record does **not** close, and the only things that stand between a chat and the
person's access beyond the proxy:

1. **Claude Code's own harness, its hooks and MCP servers run outside the sandbox**, as the
   person's process. They reach the network and hold the chat's environment, and **the identity
   is the port plus the secret**: a hook or an MCP server of the person's that reads the secret
   acts as that chat to the proxy. The proxy's allowlist still binds it, but it is not a chat's
   sandbox.
2. **A managed `allowLocalBinding`.** Where an organisation's managed settings allow a chat to
   bind a local port, a chat can stand up a listener of its own. The proxy's identity does not
   rest on the profile alone (the secret is required), but the isolation of each chat's port
   weakens, and readiness reports such a policy.
3. **A same-uid, unsandboxed process on the machine reading the secret** (from the chat's
   environment through `/proc` on Linux or `ps -E` where it is allowed, or from a core dump) can
   act as that chat until the chat ends and the secret is rotated.
4. **The person's own terminal** has their whole access. Not purlis's.
5. **A chat the person chose to run with no sandbox** (D3b): the proxy is its route, not its
   guard, and nothing stops its own use of the person's login.
6. **CI code an existing workflow runs on the chat's branch push.** A push of a branch makes the
   repository's **existing** workflows run, with the repository's secrets and the job token, on
   code the chat wrote: a script the workflow calls (`make`, `npm test`, a local action in the
   branch) is code the chat controls. The `workflow` scope and the CI-file hold (D8.3) protect the
   **workflow files**; they do not protect **what a workflow runs**. A repository whose CI runs
   untrusted branches with secrets has this hole with or without purlis, and **only the forge's
   own settings** (environments with required reviewers, secrets limited to protected branches)
   close it. The window recommends them as advice (D6), never as a condition.
7. **Labels and comments that trigger a bot or a workflow.** A label or comment can trigger a
   merge bot, or a `labeled` or `issue_comment` workflow, that merges or deploys. The proxy
   refuses a comment, review or description body matching **the known bot commands** (D3e) and a
   label write whose name matches a known merge label (`automerge`, `ready-to-merge`, a
   repository's declared list). **The rest is a named residual**: a bot or workflow with a
   command or label the proxy was not told is outside what it can know.
8. **Branch names that match a deploy trigger.** A chat chooses its branch's name, and a workflow
   that runs on `push` to `deploy/*` or `release/*` runs on it. The proxy refuses a branch name
   matching **the protected patterns the forge reports for the repo and a short fixed list**
   (`release/*`, `deploy/*`, `prod/*`, `main`, `master`); a pattern the repository's workflows use
   and nobody told the proxy is a residual. The ref commands are also read (D3f).
9. **The keychain `gh` token at the watch floor.** Where the floor leaves the person's keychain
   readable (the watch floor), a chat that reads it holds the person's `gh` token. **The egress
   proxy denies the forge hosts (D3g), so the token's use from the chat is stopped; its
   exfiltration is not**: the chat can send it to a host the person allowed. Closing it is the
   full sandbox's class-6 denial, not this record's.

### D3i. The anonymous read route: dependency fetches with the forge hosts denied

With the forge hosts denied at egress (D3g), a chat's `cargo`, `npm`, `go get`, `pip` and `git
clone` of a **public repo the person has not recorded** would fail. **An anonymous, read-only
pass-through, inside the same port and the same secret, carries them, and no credential is ever
injected on it:**

- **Unrecorded public repos only.** A recorded repo (D4) keeps the credentialed route; the
  anonymous route is chosen by `/anon/<host>/` (D3c), serves a repo that is not recorded, and
  **sends the upstream no `Authorization` of any kind**. A repo that is not public answers 404 from
  the forge and the chat sees that.
- **What it carries, and only reads:** git `info/refs` and `git-upload-pack` (never
  `git-receive-pack`); archive and tarball downloads; **release and asset downloads, including
  `uploads.github.com` and `objects.githubusercontent.com` reads** and `codeload.github.com`; the
  public **LFS** read (batch with `operation: download`, and the storage `href`s it returns, on the
  closed list below).
- **A closed list of upstream hosts**: each forge host the app knows (`github.com`,
  `api.github.com` for the one read endpoints a package manager needs, the GHES and GitLab hosts
  the person has signed in to), and their storage hosts. **No host outside the list is dialed**,
  and every address is checked by ADR 0067's local-address rule (item 4) so the route cannot be
  turned on the person's network.
- **`GET` and `HEAD` only** (and the upload-pack `POST`, which reads). Everything else is refused.
  The same canonicalisation, the same header stripping and the same audit as D3d and D3g.
- **What it is not:** not a way to read the person's private repos (no credential, so the forge
  answers as it would the world), and not a way to write.

### D3j. Pages that start a sign-in are refused

**Device-flow phishing.** A chat that can send the person to `github.com/login/device` with a code
of its choosing, or to an OAuth authorization URL, can get **the person to authorize an
application of the chat's own**. The proxy therefore **refuses, from a chat, `github.com/login/
device`, `/login/oauth/*`, `/login/device/code` and `/login/oauth/access_token`, and GitLab's
equivalents (`/oauth/authorize`, `/oauth/token`, `/oauth/device_authorization`, `/oauth/
authorize_device`)**, on the credentialed route and on the anonymous one, for **every** host of the
closed list. The person's own sign-in is made by the app on the person's press (D5), not through a
chat's route. **A chat that prints a code and a link in its output** is the same attack with no
network call: purlis cannot read that output (nothing here parses a chat), so the window's own
sign-in is the only one it ever shows, and **this is named in D6** as a residual of social
engineering.

## D4. A repo is the person's only as they confirmed it

The proxy serves a repository **the person cloned from the window, or told purlis to trust**
("Trust this clone", for a clone that already exists), as ADR 0077 amended 2026-10-29 (E3) builds
the list. A clone made from the CLI is outside it (#7196) until the person trusts it. The
confirmation records the clone, the host, **the owner/name and the numeric repo id** (a rename or
a transfer does not carry the id, and an alias that resolves to the same id is the same repo; a
lookalike with another id is not), and the **registration's client id** that signed in, so a
lookalike application is not mistaken for purlis's. Submodules and forks are recorded the same
way, on the person's press.

## D5. Provisioning is the person's press, in the window

Sign-in, the request to an organization's owners, SAML reauthorization, registering a host's
application, pinning its client id, approving a held act (D8) and trusting a clone happen **only on
the person's press in the window**, never from a chat, the CLI or a socket call. These run on the
`local-ui` scope and refuse inside a chat's tree, as `sign_in_with_grant` does
(`forge/accounts.rs`, V22c). **`purlis doctor forge` and the readiness check** (D10) run **on the person's press or the
person's own terminal only, and refuse inside a chat's tree**: they call the forge as the person.
They are **read-only** (D10.1). Every standing condition (a refused organization, a revoked
token, a SAML session) is a `Notice` with its fix, never a failed push.

## D6. What is lost without the bot, and what holds

The bot was a **separate principal**, so the forge could be told to refuse it (#7133's rulesets,
bypass lists, protected branches by actor) and its log said "the bot". With the person's own token
the forge **cannot** do either. Plainly, **the forge can no longer enforce, for a chat:**

- **A merge, a ruleset or branch-protection change, a release or tag, a default-branch change,
  repository settings, hooks, secrets and deploy keys.** The token's scope (`repo`, or GitLab's
  `api`) covers all of them. **Only the proxy's refusals, and the held acts of D8** (the app
  merges, tags, releases and pushes CI files on the person's press), stand between a chat and
  them. A hole in the proxy is a hole in the forge.
- **The audit.** The forge's log says the person did it. The app's per-chat audit (D3a) is the
  only record of which chat.
- **Anything outside the proxy** (D3h).
- **An administrator's bypass.** Where the person is an administrator and a branch rule does not
  apply to administrators, the forge will not stop their token. Nothing purlis does changes that.
- **A token that does not expire.** A GitHub OAuth app's token does not expire (D2), so the
  credential lives until it is revoked or GitHub revokes it after a year unused. It is in the keyring and in the app only; **revocation is
  the only end**, and the window shows where (Settings › Forges, and GitHub's authorized
  applications page).

**What a chat run with no sandbox adds to this list (D3b).** The person chose to run it
unconfined. purlis still gives it the proxy route, so forge works, and **cannot stop that chat's
own use of the person's login** (`gh`'s config, the keychain, git's helper): at No sandbox the proxy
is a route, not a guard. This is **the one exception** to "every chat, one approach", and it is the
person's to choose.

**Other things purlis cannot close, named so no reader finds them later:**

- **CI code an existing workflow runs on the chat's branch push, labels and comments that trigger
  a merge bot, a `labeled` or `issue_comment` workflow, branch names that match a deploy
  trigger, and the keychain `gh` token at the watch floor** (D3h.6 to D3h.9). Each has its refusal
  where one can be written and its residual where it cannot; the forge's own settings
  (environments with required reviewers, secrets only on protected branches) are the closing
  advice.
- **Device-flow phishing.** The proxy refuses a sign-in page from a chat (D3j); a chat that **tells
  the person in its own words** to open a code page and type a code is social engineering outside
  what purlis can read. The window's own sign-in is the only one it shows, and it says so.

**What the forge still enforces, whatever the proxy does:** the person's own role (a read-only
person cannot push); the **scopes not requested**: no `delete_repo`, no `admin:*`, and **no `workflow` on the everyday token, ever**: it lives
on a second token only the app uses for a held act (GitHub refuses a push of a workflow file
otherwise); an organization's
approval (D1.2); and **a user token dies with the person's membership**, where a bot's outlived it.
**Nothing here asks anyone to add anything to a bypass list.**

**#7133 closes as moot** where it asked for a forge-side lock on a bot. **A repository that wants a
forge-side backstop for its default branch** (a ruleset requiring a pull request and a review) gets
one that binds the chat as it binds the person, and purlis recommends it in the window, as advice,
never as a condition.

## D7. Roles, and forks

**The chat has the person's whole role within the chat's recorded repos, and no power the person
lacks and none outside those repos** (the proxy's list narrows it, never widens it). That
restates FI3's "gain no new power" (the amendments below): the chat gains **the person's power**
inside the proxy's list, where before it kept its own narrow identity.

| Role (GitHub / GitLab) | Fetch, pull | Push a branch the chat created | PR/MR, comment | CI read | Held acts (D8) |
|---|---|---|---|---|---|
| Admin, Owner / Owner | yes | yes | yes | yes | yes, on the person's press |
| Maintain / Maintainer | yes | yes | yes | yes | yes |
| Write / Developer | yes | yes | yes | yes | merge only if the forge lets the person |
| **Triage** | yes | **no** | comment, label, assign, open issues; **no** push, so no PR from a branch of the repo (fork, below) | yes | no |
| Read, outside collaborator without write / Reporter | yes | **no** | **comment on pull requests and issues** (read-only PR comments); open a PR from a **fork** | yes | no |
| **Guest** (GitLab) | **no in a private project** (a Guest cannot read code), yes in a public one | no | comment on issues; not MRs on code it cannot see | no | no |
| **Planner** (GitLab) | **no in a private project** | no | work items, epics, issues | no | no |
| **Custom roles** (GitHub, a base role plus permissions; GitLab custom roles) | **per the role's permissions**, which the forge reports and purlis reads at the readiness check | per the permissions | per the permissions | per the permissions | **only where the forge lets the person**; an unknown permission set is treated as **Read** for the proxy's own offers |
| **Minimal Access** (GitLab: a group member with no project access until added) | **no** in a project the person is not added to | no | no | no | no |

A role that cannot fetch gets **no git route**: the readiness view shows *"Your role on acme/app
(Guest) cannot read code. Ask an owner for Reporter or higher."* (D10) and the proxy serves the
issue and work-item routes only. **A row is the forge's decision**; the proxy never grants more than
the forge allows, and **a held act the person's role cannot perform is not offered**.

**Forks are a held act, not a route on the chat's allowlist** (D8.6). For a person who cannot
push, the window offers **Make my fork** (their press, D5): the app creates the fork **under the
person's own account only** (`make_fork`, `forge/backend.rs:285`), records it (with its id) as a
second repo of the same chat, and the chat opens the request across (`open_draft_from_fork`,
`:708`). **A chat's own fork request is refused (D3d)**, and a fork to another organization is
refused whoever asks. Supported on github.com and gitlab.com, and on GHES and self-hosted GitLab **where
the host allows forks for the person**; a host or organization that forbids them (GitHub lets an
owner disable forking of private repositories) leaves the person read-only, said in a Notice.

## D8. Held forge acts: merges, tags and releases, CI-file pushes, forks

These are **not refused dead**. They are things the person wants done, and a chat that is told
"no" with no way on is the dead end ADR 0124 §16 forbids. The pattern is ADR 0124 §7's merge hold,
**generalised**: **the chat asks; the person approves in Asks; the app performs the act with the
sign-in.** A chat never performs one. **In every case the Ask shows exactly what will happen, read
by the app from the forge and the clone and never from the chat, and the act it performs is
bound to what was shown.**

### D8.1 Merges

The proxy recognises a merge by its **request**: `PUT /repos/:o/:r/pulls/:n/merge`, the GraphQL
`mergePullRequest` and `enablePullRequestAutoMerge`, GitLab's `PUT …/merge_requests/:iid/merge`
and every other way a request merges (D3e). **How a refused `gh pr merge` becomes that ask:**

1. `gh pr merge 12` sends a request the proxy recognises as a merge of PR 12 of a recorded repo.
2. The proxy **does not forward it**. It answers the chat with a 403 whose message is one
   sentence: *"purlis holds merges. It has asked the person; the merge happens when they approve
   it in Asks."*
3. In the same step the proxy **raises the merge hold** for that PR: the repo and the PR number
   come from the request, **the head SHA, title, author, base and checks are read from the forge**,
   never from the chat (ADR 0124 §7.2), and the ask appears in Asks. No harness output is parsed:
   the ask is made from the structured request the proxy already had to understand to refuse it.
4. On **Merge**, the app calls the forge as the person (the merge method the person's press
   chose, the base the ask bound, #7213), and **the call sends the head SHA the person viewed**
   (GitHub's `sha` parameter on the merge, GitLab's `sha`): **a head that moved after the ask makes
   the forge refuse the merge** rather than land a commit nobody saw. The chat sees the PR merged
   on its next read. On **Don't merge**, nothing happens and the ask closes. **A merge always takes
   two presses** (ADR 0124 amended 2026-11-14); this record changes neither press.

### D8.2 Tags and releases

A tag is a ref push (`refs/tags/*`) or `POST …/git/refs` or `…/releases`; GitLab `POST …/repository
/tags` and `…/releases`. The proxy holds the request the same way, and **the Ask shows**:

- **For a tag:** the repo, the tag name, **the target SHA** and **whether that SHA is on the
  default branch** (read from the forge: a tag on a commit nobody merged says so in words). **An
  existing tag is never moved**: a request to create a tag that exists, to update a ref under
  `refs/tags/`, or to delete a tag is refused, not held.
- **For a release:** the same, plus the title and the notes it would publish, **and the release
  is bound to the SHA at ask time**: the app creates the tag at that SHA first and the release
  against the tag, so a branch that moves between the ask and the press does not change what is
  released. A release that creates its own tag (`target_commitish` naming a branch) is rewritten to
  the SHA the person saw.

On approval the **app** creates the tag or release as the person (#7211, #7212). A chat never
pushes a tag.

### D8.3 Pushes that touch CI files

CI files are **`.github/workflows/**`** on GitHub, and on GitLab **`.gitlab-ci.yml`, every file it
names in an `include:`, and the project's custom `ci_config_path`** (the app reads the project's
setting from the forge at each push).

**How the check works, and what it costs.** **`diffscan.rs` is a pre-commit scanner** that reads a
commit's index (SQ-16); it reads no pack and no tree. **What this needs is new code**: the app
receives the push's pack in a quarantine, **computes the tree diff of the push's new tip against
the tip the forge has** (which needs the clone's objects: the pack is a thin pack against them),
and takes the changed paths from the diff, not from the ref names or the commit messages. The same
check applies to **every other way a ref moves**: a REST `POST …/git/refs` (create-ref), `PATCH
…/git/refs/…` and the GraphQL `createRef` and `updateRef`, whose target is a commit **the proxy
resolves and diffs the same way**; and **a commit that already exists in the repository's fork
network** (GitHub shares objects between a repository and its forks, so a ref can be created at a
commit that was never pushed to this repo): the diff is computed on the **commit's tree against the
base's**, wherever the objects came from. **If the diff cannot be computed** (an object missing
from the clone, a pack too large to buffer, a remote `include:` or a variable path the app cannot
read), **the push is refused, not allowed and not guessed.**

If a path matches, **the pack is held in the app, the ref is not updated, and the Ask shows the
diff of the CI files** (the changed lines, not only their names) and the branch. On approval **the
app pushes the held pack**. **On GitHub that push uses the second token** (D1.1): the
`workflow`-scoped one, minted by its own sign-in on that press, **used only here and for no
route of a chat**; the everyday token never gets `workflow`. Because a pushed pipeline file runs
code as the person (GitLab) or with the repository's secrets (both), **a CI-file push is never
approved by default and never remembered**: each one asks. A change an `include:` resolves to a
file the app cannot read is held for the ask to say so, or refused if the diff cannot be shown.
**The hold does not cover what an existing workflow runs on a branch push** (D3h.6).

### D8.4 Where no one is present

A held act with nobody to ask (an unattended runner, ADR 0124 §6) is **refused with the sentence
"Held: nobody is here to approve it."** and queued as an ask on the person's desktop if one is
connected (ADR 0077 E4). Nothing is performed unattended.

### D8.5 What this amends

**ADR 0124 §7 items 3, 4, 5 and 9, §7.2, and the 2026-11-14 two-press rule** (this checkout's 0124
has no decision numbered 20, so the amendment names the items):

- **Item 3** (held pushes: *"A push to a protected branch, or a bare force push, is held"*;
  *"`--force-with-lease` to the chat's own branch isn't held"*): **now also tags, and pushes that
  touch CI files** are held, with the Ask contents above; the lease to the chat's own branch stays
  allowed (D3f).
- **Item 4** (*"Switching the merge hold off means purlis stops intercepting merges: the forge and
  the chat's own token decide"*): **there is no chat token, so there is nothing to fall back to**;
  the hold can be set to "ask every time", never to "let the chat merge".
- **Item 5** (*"That list, or a brokered forge token (a later design, outside these phases), is a
  precondition for offering 'purlis decides'"*): the brokered forge token **is this record's
  proxy**. **The "purlis decides" precondition never applies to a held act**: no setting, list or
  mode lets purlis approve a merge, a tag, a release, a fork or a CI-file push in the person's
  place. Where "purlis decides" is offered at all, it is for acts the allowlist already allows.
- **Item 9** (*"A stated gap … a script that merges with the chat's token gets past purlis"*):
  **with no chat token the gap shrinks to a script that merges through the proxy's route, which
  the proxy recognises by request** (D8.1). A script that calls the forge in another way has no
  credential to do it with. The remaining gap is a chat with no sandbox (D3b).
- **§7.2** (the merge row's content: *"title, author, base and checks are read from the forge"*;
  the row names the repo and the head SHA): **adds that the merge call sends the viewed head SHA**
  (D8.1), so a head that moved is refused by the forge and not only redrawn on the row.
- **The two-press rule (amended 2026-11-14)** stays: a merge takes two presses. **A tag, a
  release, a fork and a CI-file push take two presses as well**, because each is as hard to undo
  as a merge; the second press is the same "asks once more" step with the facts re-read.

The generalisation to tags, releases, forks and CI-file pushes is written as an amendment file
`docs/adr/0124/<date>-held-forge-acts.md` when this record is accepted.

### D8.6 Forks

A fork is **a held act and a press**, not a route on the chat's allowlist (D3d, D7). The chat asks
(or the window offers **Make my fork**); the Ask shows the source repo, **the person's own account
as the only destination**, and whether the host allows it. A fork request that names another
organization or namespace is refused, not held. On approval the app creates the fork as the person
and records it (D4).

## D9. Token connections: credentials where no person signs in

**No person signs in on a runner.** With the desktop present, a runner's forge calls are
**forwarded to the desktop app's proxy** (D3a): the runner holds nothing, the desktop's sign-in
serves them, ADR 0078's V9 intent (runner needs no forge credential) holds. **Headless** use (CI,
`purlis serve`, the operator's long-running host) has no desktop and **uses a token connection
only**. The proxy, its allowlist and its refusals are **identical**; only the credential's source
differs.

**Desktop forwarding: the transport and who the runner is.** The runner's own per-chat proxy
(D3b) **mints its own per-chat secret and enforces the chat's port**, exactly as on a desktop; it
does not hold the person's credential. For a call it cannot answer it **forwards over the runner's
existing authenticated connection to the desktop** (the one ADR 0078's runner already uses; **no
new listener on either side**), over that connection's TLS, **each forwarded request carrying the
runner's identity and the chat id**. The desktop **authenticates the runner by that connection's
key, applies its own recorded repos for that runner's chats** (D4) and its own allowlist, and
answers. **A runner the desktop has not paired is refused**, and a forwarded request that names a
repo the desktop did not record for that chat is refused (the runner's list is not trusted). The
connection's exact name and key handling are ADR 0078's; slice A confirms them against it before
this ships (Open item 11).

**Headless: which repos, and when.** With no press to record a repo (D4), **the repos are recorded
from the token connection's own configuration, at runner start**: a **repo list given with the
token** (`PURLIS_FORGE_REPOS`, a comma-separated list of `host/owner/name`, or the Secret's
`repos` key), else **the CI checkout's `origin`** (the one repo the job runs in). The runner's
proxy mints the per-chat secret at that start. A repo not in that list or the checkout is not
served. **Nothing is recorded by a press, because nobody is there to press.**

**What a headless chat's environment is scrubbed of, and which tokens are accepted.** The chat's
environment is **built from an allowlist, not inherited**. The proxy's process reads the
credential; the **chat never sees** `GITHUB_TOKEN`, `GH_TOKEN`, `GH_ENTERPRISE_TOKEN`,
`CI_JOB_TOKEN`, `GITLAB_TOKEN`, `PURLIS_FORGE_TOKEN*`, **the checkout's authentication**
(`actions/checkout`'s `http.<host>.extraheader` in `.git/config` and its includes, and the CI's
credential-helper entries, which purlis removes from the chat's git configuration), **and
`/run/secrets`** (the chat's sandbox does not mount it). A token is **accepted only if its type is
checked by prefix and the operator opted that type in**: `github_pat_` (fine-grained PAT), `ghs_`
(installation token and the Actions token), `glpat-` (GitLab PAT and access tokens) and `glcbt-`
or the job token's documented form; **a classic `ghp_` token and an unrecognised prefix are
refused**. **Fail closed**: a token that does not match, an ambiguous host, two tokens at the
same level (D9.2) or a repo list that cannot be read starts nothing.

### D9.1 What a token connection is, and which tokens

| Forge | Tokens a connection may be |
|---|---|
| GitHub (github.com, GHES) | `GITHUB_TOKEN` (the Actions job token), **an installation token the CI mints** for a GitHub App of the organisation's own, a **fine-grained PAT** |
| GitLab (gitlab.com, self-hosted) | `CI_JOB_TOKEN`, a **project or group access token**, a **PAT** |

A classic GitHub PAT is not offered (it is `repo`-wide with no repository selection), and is
refused by prefix (D9). A pasted
**personal access token on the desktop** (the host where no application can be registered, D1.3)
is a token connection too, with the same rules and no renewal.

### D9.2 Names, precedence, the operator Secret

- **Environment names.** `PURLIS_FORGE_TOKEN` is the token for **one** host named by
  `PURLIS_FORGE_HOST`. For several hosts, `PURLIS_FORGE_TOKEN__<HEX>`, where `<HEX>` is **the
  lowercase hex of the host's UTF-8 bytes** (`github.com` is `6769746875622e636f6d`). Round 4's
  "upper-case and every non-alphanumeric character as `_`" is **withdrawn: it was not
  collision-free** (`a-b.com` and `a.b-com` and `a_b.com` all became `A_B_COM`). Hex has one
  spelling per host. A host that differs only in case is the same host (host names are lower-cased
  first). The window shows the name for a host so no one computes it. The forges' own names are **also read** where purlis
  can tell the host: `GITHUB_TOKEN` and `GH_TOKEN` for github.com, `GH_ENTERPRISE_TOKEN` (and
  `GH_HOST`) for a GHES host, `CI_JOB_TOKEN` and `GITLAB_TOKEN` (with `CI_SERVER_URL` or
  `GITLAB_HOST`) for GitLab. Each is read **by the proxy's process** and is never put in a chat's
  environment.
- **The operator Secret** (`purlis serve` in a cluster): a Kubernetes Secret named
  `purlis-forge-tokens`, with one key per host (`github.com`, `ghe.example.com`, `gitlab.com`), the
  value the token, mounted as files under `/run/secrets/purlis/forge/<host>` or surfaced as the
  environment names above. A file is read at each use, so a rotated Secret is picked up without a
  restart.
- **Precedence**, highest first: (1) a **desktop's forwarding** where a desktop is connected (D9, a
  person's sign-in beats a shared token, and the audit then says who); (2) `PURLIS_FORGE_TOKEN__<HEX>`
  or `PURLIS_FORGE_TOKEN` + `PURLIS_FORGE_HOST`; (3) the operator Secret's file; (4) the forges' own
  names. A conflict between two at the same level is a refusal at start, naming both, never a
  guess.

### D9.3 Renewal and expiry warnings

purlis **does not renew** a token connection (it has no refresh grant). It reads the token's
expiry from the forge where the forge says (GitHub's `github-authentication-token-expiration`
response header on a fine-grained PAT; GitLab's `GET /personal_access_tokens/self` for a PAT, project
or group token), and warns: **a Notice at 14 days** (on a desktop), a `purlis doctor forge` line and
a log line at **14 and 3 days**, and a refused-request sentence naming the expired connection and
what to rotate. A token with no expiry (a `GITHUB_TOKEN`, `CI_JOB_TOKEN`) is read as bound to its
job: the proxy ends with the job. **A fine-grained PAT and a `GITHUB_TOKEN` return no
`X-OAuth-Scopes` header** (D1.1), so the connection's reach is checked **by a call** (D10.1). An installation token is renewed by **the CI that mints it**, in
the file or variable purlis rereads (D9.2).

### D9.4 What each cannot do (read from the forges' documentation as known; not re-read this round, so slice C's probe confirms each before the table is relied on)

| Token | Cannot |
|---|---|
| `GITHUB_TOKEN` | reach any repo but the one the workflow runs in; push a workflow file; trigger other workflows from its events; open or approve a PR unless the repository's Actions setting allows it |
| An installation token | act as a person (its commits and PRs are the App's); exceed the permissions the App was installed with (`workflows` is a separate permission); reach repos the App is not installed on |
| A fine-grained PAT | reach a repo outside its selection; act at all where an organization requires approval of fine-grained PATs and has not given it; outlive its expiry |
| `CI_JOB_TOKEN` | reach another project unless that project's allowlist names this one (and the allowlist **direction** matters: a job reaches projects that allow its project); act after its job ends; **call most of the API: its reach is a short list of endpoints, so a `glab mr create` or a comment through the proxy is refused by GitLab and the proxy reports it as the token's limit, not as its own refusal**; it can fetch and, where the project allows it, push to its own project's repository, and read packages and the registry |
| A project or group access token | exceed its role (a bot user) and its scopes; reach beyond its project or group |
| A GitLab PAT | outlive its expiry; act as anyone but its owner |

**A runner's merge, release or CI push is refused for nobody to approve (D8.4)** unless a desktop is
connected to take the ask. The refusals of D3d to D3f hold the same on every connection.

## D10. The readiness check, and the guides to an owner

Everything in D1 to D9 depends on a forge setting that only an owner, an administrator or the
person can change, and that fails as a 403 with a different reason each time. **The readiness check
finds each one before the first failed push and says whose act it is.** It is slice D's.

### D10.1 What is checked, and when

**When:** after a sign-in, at every clone and trust (D4), on **Check again**, when the window
regains focus with a standing condition open, and by `purlis doctor forge`. **How:** the app, as
the person, makes **read-only** requests of its own (the non-repo read list, D3d, plus
`GET /repos/:o/:r`, `git ls-remote` over the proxy, and the role read), and decides by **status
code and response header** (`X-GitHub-SSO`, `WWW-Authenticate`, a rate-limit header), with the
forge's own message shown to the person verbatim and **not used to decide** where a status and a
header can. A refusal it cannot place is a generic Notice with the forge's words, never a guess.
Nothing here parses a chat's output.

**Read-only, on the person's press or own terminal only** (D5): the check calls the forge as the
person, so it **refuses inside a chat**, and it performs no write (no fork, no ref, no comment,
nothing a failed check could leave behind). **`X-OAuth-Scopes` exists for an OAuth token and a
classic PAT only; a fine-grained PAT and `GITHUB_TOKEN` return none**, so for those the check **probes
by a call** (a read of the repo, of its default branch and of the one list a missing permission
would refuse) and says what was reached.

Per host the checks are: token valid and its scopes (`X-OAuth-Scopes`, where it exists); for each recorded org or
group: **access**, **SAML**, **OAuth restriction**, **IP allowlist**, **role**; for each repo:
**HTTP git reachable**, **LFS present** (D3f), **submodules** (D3f), **forking allowed** (D7), the
**CI config path** (D8.3) and the **default branch**; for GHES and self-hosted GitLab:
**registration present, owner shown, client id pinned** (D1.3) and what the live sign-in's
reply said of expiry (D2).

**What cannot be detected, said as such.** Some conditions return nothing a client can read, and
**the readiness view marks each "can't detect — guide anyway"** with the guide one press away,
rather than showing a green row that means nothing:

- **The OAuth app's owner** is returned by no API (D1.3): it is shown on the **forge's consent
  page**, and **its display and pin are a gating probe** (Open item 3), not a readiness row that
  could read it.
- **EMU (enterprise-managed users) policy**, and **an outside collaborator's request option being
  off**: the forge answers a refusal, with no field that says which setting caused it.
- **Some IP-allowlist cases against a restriction**: a 403 from an allowlist and a 403 from an
  OAuth restriction can read the same to a client; where purlis cannot tell, it names both.

### D10.2 The guides: one per exception

Each guide is **who acts, where, the steps, and the request text to copy**. The window shows the
guide for the person's case with the owner's steps as a message the person sends to the owner; it
**never** performs an owner's step. **Exact click paths were read from the forges' documentation as
known and are not run against a live org or instance this round: slice D confirms each against a
live one before the guide ships.**

| Exception | How it is recognised | Who acts | The steps | Request text |
|---|---|---|---|---|
| **OAuth app access restrictions** (GitHub) | `GET /repos/:o/:r` is 403 for a private repo the person is a member of, and the forge's message names the restriction | an **organization owner** | Org › Settings › Third-party access (OAuth application policy) › find *purlis* under pending requests (or the person's request) › **Grant access**; or, with no request, the owner opens the application's page on GitHub (*Settings › Applications › Authorized OAuth apps › purlis*) and presses **Grant** beside the organization | *"Please approve the OAuth application purlis (client id `<id>`, owned by `<owner>`) for `<org>`. It asks `repo` and `read:org`. Why: I use it to work in `<repo>` from purlis. Approving once covers everyone."* |
| **SAML re-authorization** (GitHub) | 403 with `X-GitHub-SSO: required; url=…` | **the person** (their own browser) | Open the URL in the header › sign in with the organization's identity provider › Settings › Applications › Authorized OAuth apps › purlis › **Authorize** beside the organization | none: the person's own step, **the one re-sign-in not caused by token life** |
| **The GitLab instance switch** (user-owned applications off) | `user_oauth_applications` off: the application cannot be registered by the person (D1.3) | a **GitLab administrator** (or a group Owner, for a group-owned application) | Admin › Applications › **New application** (instance-wide), or Group › Settings › Applications; name *purlis*, scope `api` (what the built registration asks, D1.3), device flow on (PKCE: the redirect the window shows), **not trusted**, confidential **off** | *"Please register a GitLab OAuth application for purlis on `<host>`, instance-wide, scope api, no secret needed. Send me its Application ID."* |
| **EMU / enterprise-owned organizations** | **can't detect — guide anyway** (D10.1): the org belongs to an enterprise; the person's account is a managed user; sign-in or access is refused as managed-account policy | an **enterprise owner**, not only an org owner | Enterprise › Settings › Policies / Third-party applications: allow the application for the enterprise's organizations, then the org owner's OAuth step above | the OAuth request text, addressed to *the enterprise owner of `<enterprise>`* |
| **Outside collaborator, requests disabled** | **can't detect — guide anyway**: the person is an outside collaborator and GitHub does not offer *Request* on the authorization page | an **organization owner** | the owner presses **Grant** beside the organization on the application's own page (no request is needed), or turns the request option on | the OAuth request text, plus *"I am an outside collaborator, so GitHub does not let me request it."* |
| **GitLab group SAML for git** | HTTP git is refused for a member of a group that enforces SSO for git (a 403 on `git-upload-pack` while the API works) | the **person** first (an active SAML session in the browser), then a **group Owner** if the session does not cover OAuth git | The person signs in to the group via SSO in the browser; if git is still refused, the Owner reviews *Group › Settings › SAML SSO › Enforce SSO-only authentication for Git and Dependency Proxy activity* | *"Git over HTTPS is refused for my purlis sign-in on `<group>`. Please check the SAML 'enforce SSO for Git' setting, which blocks OAuth tokens for git."* |
| **HTTP git disabled** | `git ls-remote` over HTTP is refused by the instance, the API works | a **GitLab administrator** | Admin › Settings › General › Visibility and access controls › **Enabled Git access protocols**: *Both SSH and HTTP(S)* (purlis speaks HTTP only: it holds no SSH key for a chat) | *"purlis uses Git over HTTPS only. Please allow HTTP(S) in Enabled Git access protocols."* |
| **IP allowlists** | 403 whose reason is the allow list, from the person's address; **an allowlist and an OAuth restriction can read alike, so a 403 that does not say is "can't detect — guide anyway"** and names both | an **organization or enterprise owner** (GitHub), a group Owner or administrator (GitLab) | Add the address the forge sees: the **desktop's** (the person's network, a VPN) or, for a runner with a token connection, the **runner's egress range** (D9). GitHub: Org › Settings › Authentication security › IP allow list; also *Allow access by GitHub Apps* where an installation token is used | *"Please add `<address or range>` to the IP allow list for `<org>`: it is where purlis's requests come from."* |
| **Fine-grained-PAT-only organizations** | the org requires fine-grained PATs and approves them | **nobody, for OAuth**: this policy governs PATs and does not affect an OAuth app's token, so the readiness view reports *not applicable* | none for a sign-in. For a **token connection** (D9): a fine-grained PAT must be approved by the org owner (Org › Settings › Personal access tokens › Pending requests), and a classic PAT is refused | for a PAT only: *"Please approve my fine-grained token `<name>` for `<org>`."* |

### D10.3 Re-check, and where the person sees it

Every guide ends in **Check again**: it re-runs only that row's check, shows the row green or the
next reason, and clears its Notice. A standing condition re-checks when the window regains focus, at
most once a minute. **The view:** Settings › Forges › **Readiness**, a table by host, then by
org or group, then by repo: *Ready*, *Waiting on an owner*, *Your step*, *Not applicable*, with
**Copy request**, **Open the forge's page** and **Check again** on each row; the same table is
`purlis doctor forge` (exit non-zero while any row is *Waiting* or *Your step*; `--json` for CI; **read-only, run on the person's press or own terminal, never from a chat**, D5). A
`Notice` (D5) appears for the repo a chat is blocked on, with the same guide one press away. Copy
follows `docs/ui-copy.md`.

## Amendments this record makes, verbatim

Written as files under `docs/adr/0077/`, `docs/adr/0070/`, `docs/adr/0067/` and `docs/adr/0124/`
when this record is accepted, **never as edits** of those records. The text each now reads:

**ADR 0077, FI3.** Was: *"The human's UI token never reaches an agent; agents keep their own narrow
identity (SD-7a/b, gap G3), and gain no new power from this feature before those land."* **Now:**
*"The human's UI token never reaches an agent. A chat acts as the person through the app's forge
proxy, and holds no token. Within the repos recorded for the chat it has the person's whole role
there, no more than the proxy's allowlist permits and nothing outside those repos; there is no
agent identity (SD-7a/b retired)."* The sentence "gain no new power" is restated, not kept: **the
chat has the person's role within its repos**, which is a new power relative to a bot's, and the
record says so.

**ADR 0077, V16c.** Was: *"Per-agent GitHub tokens (SD-7a) come from a GitHub App the
organisation registers itself, with charter guiding the setup; […] charter's shared GitHub App
(FW-1) is for sign-in only and its private key never ships."* **Now:** *"There are no per-agent
tokens. purlis's one OAuth registration serves sign-in and, through the proxy, a chat's calls as the
person; it has no private key. The organisation registers nothing for agents. (A GHES or
self-hosted GitLab application is registered by an owner, D1.3.)"*

**ADR 0078, V9** (the amendment is written under `docs/adr/0078/`). Was: *"PRs are opened from the
laptop, so the runner needs no forge credential (FI3 by construction)"*. **Now:** *"A runner holds
no forge sign-in. With the desktop present, its forge calls are forwarded to the desktop's proxy;
headless, it uses a token connection the operator provisions (D9). PRs are opened by the app."*

**ADR 0070 §4.** The bullet *"The sign-in token is the human's, and it never reaches a chat
(FI3)"* **stays**, and gains: *"A chat's forge call resolves, in the app's proxy, to the person's
account and no other, within the chat's recorded repos (ADR 0128)."* The clause *"(GitHub App user
tokens and GitLab OAuth tokens both expire)"* becomes *"(a GitLab OAuth token expires and is
refreshed by the app; a GitHub sign-in's lifetime is the registration's and GitHub's, read from the
sign-in's reply and never assumed; a token connection has its own expiry)"*. **The claim that a
GitHub OAuth app's token does not expire is not written into 0070**: it is read from GitHub's
documentation and is a gating probe (Open item 3), so the amendment says only what the code reads.

**ADR 0077's title.** Was: *"A forge sign-in is the human's, through one registration per forge
host, and it mints nothing for an agent"*. **Now:** *"A forge sign-in is the human's, through one
registration per forge host, and a chat uses it only through the app's proxy"*. (It still mints
nothing for an agent; "mints nothing" no longer says what the record is for.) ADR 0077 amended
2026-10-12-04 (the CLI login import) is **superseded for GitHub**, D1.2; amendment 2026-10-29-01
(an agent's GitHub token is minted from the org's App) is **superseded**.

**ADR 0067's amendments 2026-10-10-01 items 2, 4, 5 and 7 and 2026-10-10-02 item 4** (D3a lists
each, with what stands): item 2 gains *"For the forge route the port is joined by a per-chat secret
(ADR 0128 D3b)"*; item 4's exception names **a chat's own port**; item 5 gains *"except the forge
route's origin-form requests on the chat's own port, which are read"*; item 7 gains *"except the
forge route's streaming and a push held up to a size limit"*; 2026-10-10-02 item 4 is **unchanged**
and D3h.2 names a managed policy that turns it on. **No other 0067 text changes.**

**ADR 0067's amendment of 2026-11-19** (a chat on a locked repo holds no path to the person's
forge login): **the *watch*-floor keep is withdrawn** and the table's routes apply to every chat
at every floor (D3g). **`forgelock::claimed`'s mint test** (a chat claims the lock where one of its
tools mints it a token) goes with the mint.

**ADR 0124 §7 items 3, 4, 5 and 9, §7.2, and the 2026-11-14 two-press rule**: D8.5, each item
quoted there with what it now says. **No "decision 20"** is cited: this checkout's 0124 has none.

## Migration: a disposition for every item

| Item | State read 2026-11-20 | Disposition |
|---|---|---|
| **Agent identity** (`forge/agentidentity.rs`, `apptoken.rs`, `AgentIdentityFlow.tsx`, Settings › Agent identity, the Pool setup step, the `secret exec` `gh` wrapper, the App's key in the vault) | built | After the first start post-upgrade, a person who is signed in has every chat on the proxy at once; one who is not sees *Sign in to github.com*. The org's App stays installed on GitHub, untouched, and a Notice tells an owner they may uninstall it. **purlis removes its own record and, on the person's yes, the vault key**: never silently. All of it is deleted |
| **Committed `mint = "github-app"` lines** | in project files; **teammates on older builds** still read them | On the new build the line is **ignored and a Notice says so once** (*"This project asks for a GitHub App token that purlis no longer mints; chats use your sign-in"*); the line is not rewritten on the person's behalf, but a window action deletes it from the project file. A **teammate on an older build** keeps working as before (the line still mints there); the two builds do not disagree about the repo, only about the credential. The line is removed from `purlis init` templates now and from the docs; support of the key is dropped one release after this record's feature ships |
| **Forge lock slices #7205–#7216** | #7205, #7206 closed; #7207–#7216 open | See the next table |
| **Design PR #7171** (the merge and release lock is the forge credential) | open | **Closed as superseded** by this record: its premise (the forge locks merges for a bot) is gone (D6). Its facts on rulesets and bypass actors move to the research note as the advisory backstop (D6), not a design |
| **#7241** (a chat on a locked repo holds no path to the person's forge login) and **`forgelock::claimed` without the mint** | merged | **Kept and generalised**: its routes (empty `GH_CONFIG_DIR`, the helper list emptied, `GIT_CONFIG_*`, no SSH agent, the connector denies) become the every-chat baseline (D3g); `claimed`'s *claim* test is removed, so every chat is on it, **with the 0067 amendment above**. The "Not closed" rows of that amendment (a chat's own `-c credential.helper`, keychain) are what the egress denial of the forge hosts (D3g) closes, and what D3h names where it does not |
| **#7230** (GitLab works under the credentials class through a brokered `glab` route) | **open, not merged, not in the merge queue** (no auto-merge set, merge state unknown, checked 2026-11-20) | **Held, then folded**: it must not land as written, since its vault token handed to a chat is what D3 removes. Its **fallback condition is aligned with D1.3**: a vault `GITLAB_TOKEN` is read **only** where no OAuth application can be registered for the host (D1.3), then **moved to a token connection** (D9) in the keyring and the vault item offered for deletion; where OAuth is available, the window signs in instead |
| **#2053** (Runners: agent forge identity for `gh` and `glab`; SD-7a/b) | open | **Replaced** by desktop forwarding and token connections (D9). Closed as superseded; the pool and operator path is D9.2's Secret |
| **#3882** (mint a chat's GitHub App token on a GHES) | open | **Not marked met on paper.** It is met by D1.3 (a GHES registration) and D3 **only once proven**: a live sign-in against a GHES, through the proxy, with the readiness check, passes slice E's spike on that host. It stays open until then, with the proof named |
| **#7133** (the merge and release lock is the forge credential) | open | Closed as moot (D6), after its useful slices are re-parented (next table) |
| **#1351** (purlis answers git's HTTPS credential requests for sandboxed chats) | open | Closed as moot: there is no helper, nothing to answer (D3c) |
| **#7196, #7210** (a CLI clone left out of the token; refuse a mint for an unlocked repo) | open | #7196 becomes **D4's trust list** with "Trust this clone"; #7210 closes as moot (no mint) |
| **ADR 0077 amended 2026-10-12-04** (CLI login import), **2026-10-29-01** (App-minted token) | in force | Superseded (D1.2, amendments above) |

**The forge lock slices, each:**

| Slice | Disposition |
|---|---|
| #7205, #7206 | Closed and kept as built: #7205's routes are D3g's baseline; #7206's exact permission allowlist is the **model for D3d** (the proxy's allowlist is the same idea one level up), the mint it guards goes |
| #7207 (read the lock) | Re-scoped to the **readiness check's** read of a repo's rulesets (advice, D6/D10); no `claimed` state |
| #7208 (show the lock per repo; warn on the mint) | Folded into Settings › Forges › **Readiness** (D10.3); the mint warning is dropped |
| #7209 (set the lock from the window) | Closed as moot except the **advisory backstop** (D6), which is a window suggestion, not a setting |
| #7210 (refuse a mint, with migration) | Closed as moot (no mint); its migration is this table |
| **#7211 (tag as the app's act)** | **Kept, re-parented to D8.2**: the held tag the app performs on approval |
| **#7212 (release as the app's act)** | **Kept, re-parented to D8.2** |
| **#7213 (the merge ask binds the PR's base)** | **Kept, and lands first.** It is **a merge-hold hole on its own**: a retarget after the ask rides the press. D8.1 depends on it, and it needs no part of this record to merge |
| #7214 (GitLab read, show, set the lock) | Re-scoped to GitLab's side of the readiness check (D10); setting is dropped |
| #7215 (a Developer-role GitLab chat credential) | Met by D7's role table and the person's own sign-in; closed once proven on a Developer-role account |
| #7216 (the lock's live checks, nightly) | **Re-scoped to the proxy's live checks**: the nightly runs the readiness check and one spike scenario per host kind; part of phase D |

**New stores.** Each names its tier in `docs/plane-format.md` before it ships (ADR 0069): the
per-host access and refresh tokens (**Keyring**, `purlis/@forge/…`, as ADR 0077 §7); the
**per-host registration store** with its pinned client id and owner (**Machine, device-bound**,
nothing secret, rebuildable by signing in again); a token connection's secret **is the operator's
own** (a file or environment that purlis reads and never stores; a desktop-pasted PAT is a
**Keyring** item); the held-act asks (**Clone state**, rebuildable: a held pack is memory only, so
a restart drops it and the chat re-asks); the per-chat secret (memory only, not a store). The
`agent-identity-gitlab-` store of round 1 is not made.

## What the round-1 to round-3 reviewers found, and how it closes

| # | Finding | How it closes |
|---|---|---|
| 1 | GitLab self-rotation | No GitLab token is in a chat (D3) |
| 2 | CI files | GitHub: no `workflow` scope until approved (D1.1); both: the pack is read and held (D8.3) |
| 3 | Audit | App-side per chat; the forge cannot say (D6) |
| 4 | Offboarding | The token dies with the person's access (D6) |
| 5 | Lookalike App names | The registration's client id is recorded (D4) and pinned per machine (D1.3) |
| 6 | Bound by installation, not by the person | There is no installation |
| 7 | The App does not serve every collaborator | OAuth app: wider in every case (D1.1) |
| 8 | Group tokens span every project | There are none |
| 9 | A helper answers anyone who runs it | No helper; nothing to fill (D3c) |
| 10 | A remote can be rewritten | A push goes to the recorded address (D3f) |
| 11 | An attacker installs the App on a dependency | No App; only confirmed repos are served (D4) |
| 12 | Provisioning from a chat | Only on the person's press (D5) |
| 13 | Merge and ruleset writes through an allowed token | Allowlist and held acts; the forge **cannot** back them (D6, D8) |
| F3 | Round 3, both reviewers: dependency fetches break with forge hosts denied; one shared port; the secret's mechanics; `glab` carries the wrong credential; an unconfined chat | D3i anonymous read route; D3b the chat's own port and a ≥128-bit secret; D3b `glab` fixed; D3b the No-sandbox exception named |
| F3 | Headless and desktop forwarding; ambient tokens | D9: repos from the token's config, forwarding transport, scrub, prefix check, fail closed |
| F3 | Held acts' Asks; CI-file bypasses; `workflow` scope; "purlis decides" | D8: what each Ask shows; tree diff of the new tip; a second token; D8.5 |
| F3 | Residuals; device-flow phishing; GraphQL; fork; force-with-lease; detectability; facts | D3h.6 to .9, D3j, D3e, D8.6 and D3f, D10.1, D1.1 to D2 and D9.2 |
| R2 | The round-2 findings (security, feasibility, coverage) | Mapped one by one in the pull request's round-4 comment |

## The person's requirement, point by point

| Requirement | Where | Exception, named |
|---|---|---|
| **One approach** | One sign-in, one credential in the keyring, one proxy on **each chat's own port**, every chat at every floor, on every host (D3b, D3g) | **A chat the person chose to run with no sandbox** (D3b): it gets the same proxy route, so forge works, and purlis **cannot stop its own use of the person's login**; the proxy is its route, not its guard |
| **Personal repos** | D1.1: own and collaborator | none |
| **Organization repos** | D1.1, D1.2, D10 | **an organization with OAuth app access restrictions needs an owner's one approval** (GitHub); the person gets the guide and the request text (D10.2). **EMU, a collaborator's disabled requests and some IP-allowlist cases cannot be detected**: the guide is shown anyway (D10.1) |
| **Self-hosted, enterprise** | D1.3: an **org- or group-owned** application, registered by an owner, **its owner read off the forge's consent page by the person** (no API returns it), pinned per machine; D10.2 for EMU | an owner or administrator must register it; where nobody will, or the consent page does not show the owner, a token connection (D9) with no automatic renewal |
| **One login** | Once per forge host (and account, D2) | **SAML** reauthorization, **revocation**, **GitHub's one-year-unused revocation**, and a GitLab refresh token unused past its limit (D2) |
| **Auto renewal, no interaction** | D2: **GitHub's OAuth-app token does not expire (from documentation, a gating probe), so there is nothing to renew there**; the app refreshes GitLab's | a token connection (none); revocation; the same SAML case; the one-year rule |
| **Runners and CI** | D9: desktop forwarding over the runner's authenticated connection, else a token connection; repos recorded from the token's own config; chat environment scrubbed and token types checked by prefix | a runner with no desktop cannot approve a held act (D8.4); **`CI_JOB_TOKEN` reaches only a short list of GitLab endpoints** (D9.4), so most `glab` writes are refused by GitLab, not by purlis; `GITHUB_TOKEN`'s limits (D9.4) apply |
| **A chat can merge, tag, release, change CI, fork** | D8: asks that show exactly what will happen, approved in Asks, performed by the app with the SHA bound; a CI-file push on GitHub uses a **second, `workflow`-scoped token** the chat's routes never see | none held dead. **Not closed:** what an existing workflow runs on the chat's branch push, merge-bot labels and comments the proxy was not told, deploy-trigger branch names it was not told, the keychain `gh` token at the watch floor (use stopped, exfiltration not), and a chat's own words asking the person to open a sign-in page (D3h.6 to D3h.9, D3j). **A CI-file push on GitHub is not offered if GitHub folds the second token's scope into the first** (D1.1) |
| **Dependencies still fetch** | D3i: an anonymous read-only route for unrecorded public repos, release and asset downloads and public LFS, with no credential ever injected; recorded repos keep the credentialed route | a private unrecorded repo is refused, as it would be to anyone |

## Consequences

- A person signs in once per host, and every chat in the repos they confirmed pushes branches,
  pulls, opens requests and reads CI as them. No `gh auth login`, no pasted token, **no
  import of the CLI's login**.
- **The app is on the path of every forge call a chat makes**, and so is a headless proxy where
  there is no app.
- **purlis keeps no private key, no server and no bot.** It publishes two registrations it already
  has (their client ids ship in a release) and one an owner of a host makes for their organisation.
- **A hole in the proxy is a hole in the forge** (D6). The proxy's allowlist is the record's main
  cost, and its review should reread Q331.
- **A chat the person runs with no sandbox** keeps the proxy route and has no guard against its
  own use of the person's login (D3b): the one exception to one approach, said in D6.
- **The chat has the person's whole role** in its repos, within the list. That is stronger than
  the bot it replaces and is said in FI3's amendment, not hidden.

## Open items (nothing here is proven live)

1. **`gh` and `glab` through the proxy** (D3c), slice E's spike, gates the build: one real `gh`
   and one real `glab`, on macOS and Linux, each through `pr create`, `pr checks`, `run view`, a
   paginated list and a GraphQL query, with the wrapper clearing `NO_PROXY`. If `gh` fails, the gap
   is named as `gh`'s alone.
2. **The Linux sandbox's per-chat port pair** (D3b): confirm it is #1664's mechanism.
3. **GitHub, a gating probe (slice C): what the live sign-in's reply says of expiry (the record
   expects no `expires_in` and no `refresh_token` for an OAuth app, from documentation; "Expire
   user access tokens" is read as a GitHub App feature); that GitHub keeps a second,
   `workflow`-scoped token separate from the everyday one (D1.1); that an owner's approval widens
   an existing token; the exact refusal an unapproved organization returns; whether the consent
   page shows the application's owner on github.com, on a GHES release and on a self-hosted
   GitLab (D1.3).** **Whether GHES behaves the same.**
4. **GitHub's SAML and enterprise-owned organizations**: what a user token needs per org, and the
   guides' click paths against a live org (slice D).
5. **The CI-file hold buffers a pack** (D8.3): a size limit and a latency, to be set.
6. **The GraphQL allowlist** (D3e) is a list someone keeps; its first list is slice A's.
7. **Token-connection limits** (D9.4): each row is confirmed by slice C's probe.
8. **Two forge hosts in one chat** works for git and `glab`; for `gh` the wrapper picks the host
   from the working directory (D3c), and is the first thing the spike measures.
9. **ADR 0124 has no "decision 20"** in this checkout; D8.5 names the items (§7 items 3, 4, 5, 9
   and §7.2) and the amendment file cites those.
10. **LFS** is refused (D3f); its support is its own record. **Several accounts on one host**, a
    person who leaves an organization and a revoked authorization each show as a Notice with a fix.
11. **Desktop forwarding's channel** (D9): the runner's existing authenticated connection is named
    from ADR 0078 by reading, not by running; slice A confirms the key, the pairing and that no
    new listener is needed before the forwarding ships.
12. **`glab`'s credential header, `gh`'s and git's** (D3b) are read from their documentation and
    sources, and slice E's spike confirms each reaches the proxy as D3b says.
13. **The anonymous route's closed host list** (D3i) and **the merge-bot and label lists** (D3e)
    are lists someone keeps; their first versions are slice A's.
