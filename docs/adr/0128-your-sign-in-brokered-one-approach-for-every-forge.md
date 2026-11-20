# Your sign-in, brokered: one approach for every forge

**Proposed 2026-11-20, round 3.** Round 1 (numbered 0127 there; 0127 is the project-policy ADR on
`m75-policy`) proposed a per-owner App and GitLab project tokens, and four reviewers rejected it.
Round 2 proposed per-user tokens as the base and kept the bot as a second, opt-in mode; the person
rejected two modes. This round is **one approach**. The facts and their sources are in
[`docs/research/one-login-forge-identity.md`](../research/one-login-forge-identity.md), section 7.
Nothing here is built, and nothing was run against a live forge. What was read and what was
tried is said where each claim is made.

## The requirement

Someone has a personal repo, an organization repo, a repo on GitHub Enterprise Server (GHES), on
gitlab.com or on a self-hosted GitLab. **Anyone with access to it** signs in to their own forge
account **once per forge host**, clones the repo as a project and works. The chat runs `gh`, `glab`
and native `git fetch`, `pull` and `push`, opens pull and merge requests, comments and reads CI.
The person types no command and no token, does no setup, and **is never asked to sign in again
because a token ran out**. One approach covers all of it, and it **replaces** Agent identity (the
bot App), the GitLab vault tokens (#7230) and the brokered push's use of the CLI login.

## The decision, in one paragraph

**The person signs in once per forge host with purlis's own sign-in (ADR 0077). purlis keeps
that credential in the keyring, in the app only, and renews it itself. A chat holds no token. A
forge proxy in the app acts as the person for the chat's own repos only**: git over HTTPS and the
forge API (`gh`, `glab`), exact host, port and path; merges, releases, rulesets, administration
and every other repo are refused; a push goes only to the recorded address. There is no bot, no
installation, no project token and no second mode.

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
| An outside collaborator in a restricted organization | requests only if the owner allows requests | the same limit |

The OAuth app is **at least as wide in every row, and strictly wider in two**. A GitHub App is
the better tool for a bot, and there is no bot here. What the OAuth app costs: its token is
scoped by `repo`, not by repository, and "you can't scope source code access to read-only"
(GitHub, *Authorizing OAuth apps*). That is the proxy's job to narrow (D3), as it would have
been for any App user token.

**Scopes: `repo` and `read:org`, and nothing else.** Not `workflow` (GitHub then refuses a push
that adds or changes a workflow file, which keeps the CI-file protection on GitHub for free), not
`delete_repo`, not `admin:*`, not `write:packages`. These are real forge-side limits that hold
even if the proxy has a bug (D6). The sign-in also asks for `offline_access`, so the token
expires (D2).

### D1.2 The one case an owner decides, stated exactly

**An organization that has enabled OAuth app access restrictions** (on by default when an
organization is created: GitHub, *About OAuth app access restrictions*) **refuses an unapproved
OAuth app's reach into the organization's private resources until an owner approves it.** Nothing
purlis does removes this, and no design on GitHub removes it, for an App or an OAuth app. What
happens:

1. At sign-in, GitHub's authorization page lists the person's organizations and, for a restricted
   one, offers the person **a request to the owners**. It is GitHub's page and the person's press.
2. Until an owner approves, the app reaches only the organization's **public** resources. At
   clone, purlis asks the forge as the person, sees the refusal, and shows one Notice:
   *"acme has not approved purlis. Ask an owner to approve it: [Open GitHub's request page]."*
   Public repos, and the person's own and other organizations' repos, work meanwhile.
3. The owner approves **once per organization, for everyone**. Then every member, and every
   outside collaborator the owner lets request, works with no further act. The existing token
   gains the organization's access (expected from how approval is described; to be confirmed in
   the first live probe).

**There is a way past it with no owner, and it is the person's choice, not the default.** GitHub
lists "GitHub CLI" among its *privileged OAuth apps*, which a person may authorize despite a
restriction. A person who has `gh auth login`'d can **import that login** (already built, ADR 0077
amended 2026-10-12-04); the app then holds it like any sign-in (keyring, never in a chat). It is a
token minted by another vendor's app, so purlis **never uses GitHub CLI's client id itself**.

Two more cases that look like this one and are not:

- **SAML single sign-on organizations.** The person must have an active SAML session when
  authorizing, and after an owner enforces SSO "you must reauthorize any OAuth apps". This is
  the person's own browser step (GitHub, *About authentication with SAML single sign-on*), and
  it is the one place the person **signs in again for a reason that is not the token's lifetime**.
  The Notice says so and opens the page.
- **An account GitHub has flagged** cannot authorize any third-party app. Not purlis's to fix.

### D1.3 The other hosts

| Host | Sign-in | Registered by | Notes |
|---|---|---|---|
| github.com | The existing OAuth app, device flow | purlis (built) | scopes grow; one re-consent for existing sign-ins |
| gitlab.com | The existing OAuth application, PKCE | purlis (built) | scopes `api`, `write_repository`, `read_user` |
| **GHES** | An OAuth app registered once on the host | **any user of that host**: GHES lets a person register one under their own account ("under your personal account or any organization you have administrative access to"); **no administrator**. They tick *Enable Device Flow* | its client id is shared (below) |
| **Self-hosted GitLab** | A user-owned OAuth application, the device flow (17.1 and later; generally available 17.9), else PKCE | **any user of that host**, from *Edit profile › Access › Applications*; **no administrator** | its client id is shared (below) |

**Registered once per server, shared.** An application belongs to the person who registered it
and **any user of that host can authorize it**; so one person registers it, the window shows the
values to enter (name, scopes, and, for PKCE, the redirect), and the **client id**, which is
public, is given to purlis and shared with the team by the workspace (Machine tier, syncable).
Everyone else on that host signs in with nothing to register.

**Where it cannot be registered by a user:** a GitLab administrator can turn off user-owned
applications (the `user_oauth_applications` setting; group-owned applications are unaffected, so a
group Owner can register one for the group). Where neither is possible, the person pastes a
**personal access token** and purlis uses it only in the app. That is the one host-imposed
exception to automatic renewal (D2).

Where GitLab 19.6 or later ships its own `glab` application on a self-managed instance, it is
**glab's** application, not purlis's, and purlis does not use its client id.

## D2. purlis renews the credential; when the person signs in again

The **app alone** refreshes (ADR 0077 §3: one process may rotate a single-use refresh token).
It refreshes before expiry, and once on a 401. A closed app misses nothing: the refresh token
outlives the access token.

| Host | Access token | Refresh | Refresh needs a secret? |
|---|---|---|---|
| github.com, GHES | **8 hours**, as `offline_access` makes it | a refresh token, **valid 6 months from last use**, replaced on each refresh | **No**, "unless the token was generated using the device flow"; ours is |
| gitlab.com | 2 hours (7200 s) | a refresh token "can be used even after the access token itself expires"; each refresh replaces both | No: PKCE, no client secret |
| Self-hosted GitLab | 2 hours by default (an administrator may change it, 19.1 and later) | the same | No |
| GitLab with only a pasted PAT | the PAT's own expiry | **none** | |

GitHub's page describes the expiry as a setting or an `offline_access` request; **whether GHES
3.22 honours `offline_access` was not read.** If a host returns no refresh token, its token does
not expire, there is nothing to renew, and the credential is simply longer-lived.

**The person signs in again only when**: (1) a refresh token is **unused for six months**
(GitHub); (2) they or an owner **revoke** the authorization, or a GitLab administrator
invalidates it; (3) a **SAML** organization needs reauthorization (D1.2); (4) a **GitLab PAT**
reaches its own expiry (the PAT fallback only); (5) the forge account is changed or flagged.
Every one of these shows as a Notice with the fix in one press (D5), never as a failed push. Under
normal use none occurs.

## D3. The forge proxy

A chat holds **no token**. Three tools reach the forge for it, and all three reach one
app-side proxy.

### D3a. What it reuses, and what is new

**Reused (built):** ADR 0067's **per-chat egress proxy**: one per chat, on a pair of ports of its
own, with the sandbox profile allowing a chat's network traffic only to it (amended 2026-10-10,
#1664), deciding by the core decision module and holding unknown hosts to ask the person (#1666).
ADR 0067 §3 already names that proxy "the long-term home" of a broker that releases a secret
"only to the hosts it is bound to". Also reused: `forge/refresh.rs` and `device.rs`/`pkce.rs` (the
sign-in and its refresh), `mergehold.rs`, `diffscan.rs`, the keyring items of ADR 0077 §7 and the
recorded clone list of ADR 0077 amended 2026-10-29 (E3).

**Not built:** the key proxy of #6762 and #6812 is specified and has no code in `crates/` at
`4568a3c7b4`. **This record's proxy is therefore new code**, as a *route inside the egress
proxy*, not a second listener:

1. the **virtual forge hosts** (D3c) and the rewrite from them to the real forge over TLS;
2. the **credential injection**: the proxy removes every credential header the chat sent and
   adds the person's;
3. the **exact-match rule** on the chat's recorded repos (D3f);
4. the **allowlists**: REST routes, GraphQL mutations, git ref commands (D3f);
5. the **response rewrite**: `Link` headers, which `gh` follows blindly (D3c);
6. the **audit**: chat id, repo, endpoint, verdict.

### D3b. git

The chat's git configuration is passed in the environment, as #7241 does, and the clone's
`.git/config` is not touched:

- `url.http://github.localhost/.insteadOf = https://github.com/` (and the `ssh://` and scp forms),
  so a push or fetch is an HTTP request to a **virtual host**;
- `http.http://github.localhost/.proxy = http://chat:<secret>@127.0.0.1:<chat's port>`: git
  sends it through the chat's egress proxy.
- **No credential helper at all.** The helper list stays empty as #7241 makes it. The proxy adds
  the person's credential after it has checked the request, so **`git credential fill` yields
  nothing**: stronger than yielding a sentinel, and the sentinel's one use (D3d) is only for the
  tools that will not run without a token. (Round 2 had a sentinel-answering helper; this is
  better, and it is the one change from the brief's wording.)
- The hop from the chat to the proxy is plain HTTP on a loopback port the profile opens to that
  chat alone. The hop from the proxy to the forge is TLS by the app, with the system's roots. The
  person's token is on neither the chat's side nor the loopback.

**Not proven.** git's libcurl proxying a `*.localhost` name was not run (this session's sandbox
refuses loopback, D3d). **Fallback, which depends on nothing about proxy variables:** rewrite
straight to the loopback address, `http://127.0.0.1:<port>/github.com/<owner>/<repo>.git`, with
the host as the first path segment. The cost is that `gh` can no longer read the host from the
remote, and `GH_REPO` carries it (D3c).

### D3c. `gh` and `glab`

Go on macOS reads no `SSL_CERT_FILE`, so a proxy that must present a purlis certificate to a
Go client is **rejected** on macOS: it would need a purlis CA in the person's keychain, trusted by
every program they run, behind an administrator prompt. The mechanism instead **avoids TLS to the
chat's tools**.

**`gh`: its own localhost mode.** What was read in `gh` 2.69.0 here (the binary's strings, its
`help environment` and `help config`, one run with the sandbox's network off):

- the binary holds `github.localhost`, `http://api.%s/` and `http://api.%s/graphql` beside the
  usual `https://api.%s/` and `https://%s/api/v3/`;
- `GH_HOST=github.localhost` made it report `error connecting to api.github.localhost`, a plain
  `api.`-prefixed host with no `https://…/api/v3/` URL, where `GH_HOST=ghe.example.test` printed
  `Get "https://ghe.example.test/api/v3/user"`.

That is the shape of a plain-HTTP, `api.github.localhost` mode. The design:

- `GH_HOST=github.localhost`, `GH_TOKEN=<sentinel>` (D3d), and the chat's `HTTP_PROXY`, which ADR
  0067 already points at the chat's egress proxy. Go sends the plain-HTTP request to the proxy by
  name, so **no TLS and no local name lookup**.
- The proxy maps `api.github.localhost` to `https://api.github.com/` and `.../graphql`. **For
  GHES, the same virtual host maps to `https://<host>/api/v3/` and `/api/graphql`**: `gh`
  has no plain-HTTP mode for an enterprise host, and this gives it one. The host is chosen by the
  port the request arrived on (one port per forge host the chat has a repo on, all in the chat's
  profile); the chat's `gh` wrapper, which purlis already writes for each chat, picks the port
  from the working directory.
- The proxy rewrites **`Link` response headers** from the real API to the virtual host, or
  `--paginate` and every list would leave the proxy.
- `gh` finds its repository from the remote's host; the remote reads `github.localhost` by D3b, so
  it matches. In the fallback, `GH_REPO=github.localhost/<owner>/<repo>` is set by purlis.

**`glab`: documented, with no trick.** `glab` 1.119.0's own `config --help` lists, with
environment variables: `host` (`GITLAB_HOST`), `api_host` (`GITLAB_API_HOST`, "hostname or
hostname:port"), `api_protocol` (`GLAB_API_PROTOCOL`, `http` or `https`), `token` (`GITLAB_TOKEN`),
and `ca_cert` (`GLAB_CA_CERT`, a CA PEM). The design sets
`GITLAB_HOST=gitlab.localhost`, `GITLAB_API_HOST=127.0.0.1:<chat's port>`,
`GLAB_API_PROTOCOL=http`, `GITLAB_TOKEN=<sentinel>`: the API goes to the loopback port as plain
HTTP, and the repo's identity (from the remote, D3b) is `gitlab.localhost`. Every setting is from
the tool's own text. (`GLAB_CA_CERT` is the TLS alternative, and is not needed.)

**What is proven and what is not.** Proven: the settings exist, on the versions named. **Not
proven: that a live `gh` or `glab` completes `pr create`, `pr checks`, `run view`, a paginated
list or a GraphQL query through the proxy.** This session's sandbox refused a loopback listener
and a lookup of `api.github.localhost`, so the run could not be made, and it was not worked
around. **That run is the first task of the build and it gates it.** Known risks to it: `gh`
treating `github.localhost` with github.com's feature set when the upstream is GHES; a `gh`
command that fetches from another host (`gh run download`, release assets, gists): refused, and
the Notice says which command; the second host of a chat that has repos on two. If `gh` fails the
run, **the gap is named as `gh`'s and `git` and `glab` still ship**; no other mechanism (a
purlis CA, a shim that reimplements the API, the token in an environment) is acceptable.

### D3d. Who is asking: a per-chat port, in the profile, plus a per-chat secret

Three candidates were weighed:

1. **A kernel read of the connecting process** on a TCP loopback socket: there is no peer
   credential, and `purlis-same-user` has no socket-table read for it. Rejected.
2. **A unix socket**, which has a peer credential (`hookwire/reach.rs` already reads one). `gh`'s
   `http_unix_socket` setting still speaks TLS over it, `git` has no setting for it, and `glab`
   has none. Rejected.
3. **A per-chat port the chat's sandbox profile alone allows, and a per-chat secret.** Chosen.

Observed in this session: the sandbox refuses `127.0.0.1:18080` for this chat
("purlis's sandbox does not allow 127.0.0.1:18080") and refuses a listener (`bind`: *Operation not
permitted*). So a locked chat **can reach only the ports its own profile lists, and cannot open
one of its own**. ADR 0067 (amended #1664) gives each chat its own pair of ports. A chat therefore
**cannot connect to another chat's proxy, and cannot stand up a fake one**: the connection's
destination is the identity, with no value from the chat deciding it.

The second factor protects a port from any other process on the machine. Each chat has a random
**secret**, in the app's memory and the chat's environment only, worth nothing anywhere else. It
travels as `Proxy-Authorization` (git, `gh` through `HTTP_PROXY`) or as the token's value
(`gh`, `glab`: the "sentinel"). A request is served only if it arrives on chat X's port **and**
carries chat X's secret. Anything else is refused as IN_DOUBT, and logged.

A chat's own subprocesses share its identity, which is correct: they are the chat. A chat that is
**not sandboxed** has no such guarantee, so **an unsandboxed chat gets no proxy route and no
forge access** (ADR 0067's lock is the precondition). The Linux sandbox must offer "exactly one
loopback port per chat"; ADR 0067's #1664 already needs it for the egress proxy, so this record
adds no requirement, but Linux's wording is Open item 2.

### D3e. What the proxy does and refuses

- **Exact match** on scheme, host, port and **normalised path** against the chat's recorded repos
  (D4). Percent-encoding decoded once; `.`, `..` and `//` refused; case folded where the forge
  folds it. No prefix match: `acme/app` is not `acme/app-secrets`.
- **An allowlist of write operations, not a list of bad ones** (Q331 found a list is never
  complete): REST routes; GraphQL parsed, every mutation not listed refused. Allowed: branch push,
  pull and merge request create and update, comments and reviews, issues, labels, reading CI and
  its logs, forking (D7). **Refused whatever the token could do:** merge (REST, GraphQL and
  auto-merge), ruleset, branch-protection and default-branch changes, tags and releases,
  creating, moving or deleting the default or a protected branch, repository, hook, secret and
  deploy-key writes, and any repo not recorded.
- **Git over HTTP:** the proxy reads the ref commands that begin a push and refuses the default
  branch, a protected branch, every tag and any ref the person's role does not allow. **A push
  goes only to the address recorded for the clone**, passed to git explicitly, never to what
  `origin` resolves to now. On GitLab a pushed `.gitlab-ci.yml` runs a pipeline as the person; the
  proxy refuses such a push by reading the pack's changed paths in a quarantine (`diffscan.rs`),
  which costs a buffered pack (Open item 5).
- **Every answer is audited**, app-side.
- **If the app is gone, a chat's forge calls fail with one sentence.** Offline git is unchanged.

**The brokered `purlis worktree push` is the app's own act**, not a route through the proxy: it
uses the same stored credential, goes to the recorded address, and **stops reading the person's
`gh` or `glab` login** (`forge::helper_for`, `forge.rs:806-823`).

## D4. A repo is the person's only as they confirmed it

The proxy serves a repository **the person cloned from the window, or told purlis to trust**
("Trust this clone", for a clone that already exists), as ADR 0077 amended 2026-10-29 (E3) builds
the list. A clone made from the CLI is outside it (#7196) until the person trusts it. The
confirmation records the clone, the host and the repo **id** (a rename or a transfer does not
carry it). There is no longer an App id to record, since there is no App; the **registration's
client id** is recorded instead, so a lookalike application is not mistaken for purlis's.

## D5. Provisioning is the person's press, in the window

Sign-in, the request to an organization's owners, SAML reauthorization, registering a host's
application and trusting a clone happen **only on the person's press in the window**, never from
a chat, the CLI or a socket call. These run on the `local-ui` scope and refuse inside a chat's
tree, as `sign_in_with_grant` does (`forge/accounts.rs`, V22c). Every standing condition (a
refused organization, an expired refresh token, a SAML session) is a `Notice` with its fix,
never a failed push.

## D6. What is lost without the bot, and what holds

The bot was a **separate principal**, so the forge could be told to refuse it (#7133's rulesets,
bypass lists, protected branches by actor) and its log said "the bot". With the person's own token
the forge **cannot** do either. Plainly, **the forge can no longer enforce, for a chat:**

- **A merge, a ruleset or branch-protection change, a release or tag, a default-branch change,
  repository settings, hooks, secrets and deploy keys.** The token's scope (`repo`, or GitLab's
  `api`) covers all of them. **Only the proxy's refusals, and the merge hold** (`mergehold.rs`: the
  app merges, on the person's press) stand between a chat and them. A hole in the proxy is a hole
  in the forge.
- **The audit.** The forge's log says the person did it. The app's per-chat audit (D3e) is the
  only record of which chat.
- **Anything outside the proxy.** The person's own terminal has their whole access.
- **An administrator's bypass.** Where the person is an administrator and a branch rule does not
  apply to administrators, the forge will not stop their token. Nothing purlis does changes that.

**What the forge still enforces, whatever the proxy does:** the person's own role (a read-only
person cannot push); the **scopes not requested**: no `delete_repo`, no `workflow` (a push of a
workflow file is refused by GitHub), no `admin:*`; an organization's approval (D1.2); and **a
user token dies with the person's membership**, where a bot's outlived it. The merge hold stays
the answer to merges. **Nothing here asks anyone to add anything to a bypass list.**

**#7133 closes as moot.** There is no bot to lock. **A repository that wants a forge-side backstop
for its default branch** (a ruleset requiring a pull request and a review) gets one that binds the
chat as it binds the person, and purlis recommends it in the window, as advice, never as a
condition.

## D7. Roles, forks

The chat has **exactly the person's role** (the token is theirs) and no more than the proxy's
list allows.

| Role | Fetch, pull | Push a branch | PR/MR, comment | CI read | Merge |
|---|---|---|---|---|---|
| Owner, admin, maintainer | yes | yes (not the default or a protected branch) | yes | yes | the person's press only |
| Write, Developer, member, outside collaborator | yes | yes (the same) | yes | yes | the person's press only |
| Read-only, Reporter | yes | **no** (the forge refuses) | comment on issues; open a PR from a **fork** | yes | no |

**Forks.** For a read-only person the window offers **Make my fork** (their press, D5): the app
creates the fork under the person's own account (`make_fork`, `forge/backend.rs:285`), records it
as a second repo of the same chat and the chat opens the request across (`open_draft_from_fork`,
`:708`). Supported on github.com and gitlab.com, and on GHES and self-hosted GitLab **where the
host allows forks for the person**; a host or organization that forbids them (GitHub lets an owner
disable forking of private repositories) leaves the person read-only, said in a Notice.

## Migration

| What exists | What happens | What is deleted |
|---|---|---|
| **Agent identity** (the org bot App): `forge/agentidentity.rs`, `apptoken.rs`, `AgentIdentityFlow.tsx`, Settings › Agent identity, the Pool setup step, `mint = "github-app"`, the `secret exec` `gh` wrapper, the App's key in the vault | On the first start after the upgrade a person who is signed in has every chat on the proxy at once. A person who is not sees one Notice: *Sign in to github.com.* The org's App stays installed on GitHub, untouched, and a Notice tells an owner they may uninstall it (their act). **purlis removes its own record and, on the person's yes, the vault key**: never silently | All of the left-hand column, the manifest flow and its loopback listener, `public: false` |
| **#7230 GitLab vault tokens** (`GITLAB_TOKEN` and `GITLAB_HOST` from a vault) | The token becomes the **PAT fallback** for that host only if no sign-in exists, and is **moved to the keyring**; where the host allows OAuth, the window signs in and offers to delete the vault item | The handing of a vault token to a chat; the route #7230 added |
| **CLI-login pushes** (`purlis worktree push` through `forge::helper_for`) | Uses the stored credential and the recorded address (D3e) | `helper_for`'s `gh`/`glab` CLI branch. The CLI login's **import** (ADR 0077 amended 2026-10-12-04) stays: it is how the person brings a login in, not a way a chat reaches it |
| **The chat's own helper** (ADR 0070 amended 2026-10-12-02) | Unchanged for the window's git; a chat's helper list stays empty | Nothing |

**Written as amendment files when this record is accepted** (never edits of those records), under
`docs/adr/0077/` and `docs/adr/0070/`: FI3 and V16c (agents act as the person through the app and
never with the person's token in hand; there is no agent identity), SD-7a/b and amendment
2026-10-29-01 (superseded), and the wording in 0070. **Issues:** #1351 and #7133 close as moot;
#2053 (SD-7a/b) closes as superseded; #3882 (GHES) and #7215 (a Developer-role GitLab credential)
are met by D1 and D7; #7196 and #7210 become the trust list of D4; #7230 is met by the
migration.

**New stores.** Each names its tier in `docs/plane-format.md` before it ships, as ADR 0069
requires: the per-host access and refresh tokens (**Keyring**, `purlis/@forge/…`, as ADR 0077 §7);
the shared **client id per self-hosted host** (**Machine, syncable**, nothing secret); the per-chat
secret (memory only, not a store). The `agent-identity-gitlab-` store of round 1 is not made.

## What the round-1 reviewers found, and how it closes

| # | Finding | How it closes |
|---|---|---|
| 1 | GitLab self-rotation | No GitLab token is in a chat (D3) |
| 2 | CI files | GitHub refuses a push of a workflow file for lack of `workflow` (D1.1); on GitLab the proxy reads the pack (D3e) |
| 3 | Audit | App-side per chat; the forge cannot say (D6) |
| 4 | Offboarding | The token dies with the person's access (D6) |
| 5 | Lookalike App names | The registration's client id is recorded (D4) |
| 6 | Bound by installation, not by the person | There is no installation |
| 7 | The App does not serve every collaborator | OAuth app: wider in every case (D1.1) |
| 8 | Group tokens span every project | There are none |
| 9 | A helper answers anyone who runs it | No helper; nothing to fill (D3b) |
| 10 | A remote can be rewritten | A push goes to the recorded address (D3e) |
| 11 | An attacker installs the App on a dependency | No App; only confirmed repos are served (D4) |
| 12 | Provisioning from a chat | Only on the person's press (D5) |
| 13 | Merge and ruleset writes through an allowed token | Allowlist and merge hold, and the forge **cannot** back them (D6) |

## The person's requirement, point by point

| Requirement | Where | Exception, named |
|---|---|---|
| **One approach** | One sign-in, one credential in the keyring, one proxy, on every host | none |
| **Personal repos** | D1.1: own and collaborator | none |
| **Organization repos** | D1.1, D1.2 | **an organization with OAuth app access restrictions needs an owner's one approval** (GitHub). The person is told whose act is waiting |
| **Self-hosted, enterprise** | D1.3: GHES and self-hosted GitLab, registered once by any user, no administrator | a GitLab administrator who turned off user-owned applications leaves a PAT, with no automatic renewal |
| **One login** | Once per forge host | **SAML** reauthorization, and a refresh token unused for six months |
| **Auto renewal, no interaction** | D2: the app refreshes | the PAT fallback; revocation; the same two |

## Consequences

- A person signs in once per host, and every locked chat in the repos they confirmed pushes,
  pulls, opens requests and reads CI as them. No `gh auth login`, no pasted token.
- **The app is on the path of every forge call a chat makes.**
- **purlis keeps no private key, no server and no bot.** It publishes two registrations it already
  has (their client ids ship in a release) and one the people of a host make for themselves.
- **A hole in the proxy is a hole in the forge** (D6). The proxy's allowlist is therefore the
  record's main cost, and its review should reread Q331.

## Open items (nothing here is proven live)

1. **`gh` and `glab` through the proxy** (D3c): the first task, and it gates the build. One real
   `gh` and one real `glab`, on macOS and Linux, each run through `pr create`, `pr checks`,
   `run view`, a paginated list and a GraphQL query. If `gh` fails, the gap is named as `gh`'s
   alone.
2. **The Linux sandbox's one-port-per-chat** (D3d): confirm it is the same mechanism as #1664.
3. **GitHub: that an owner's approval widens an existing token**; **the exact refusal an
   unapproved organization returns** (how the window recognises it); **GHES and `offline_access`**.
4. **GitHub's SAML and enterprise-owned organizations**: what a user token needs per organization.
5. **The GitLab CI-file refusal** buffers a pack: a size limit and a latency, to be set.
6. **A GraphQL operation allowlist** is a list someone keeps.
7. **Unattended runners** (ADR 0077 E4; #7192): a runner has no person present. The proposal is
   that **a runner's host runs its own app and proxy, and the person signs in once on it** (one
   login per host *per machine that runs chats*), so there is no second approach. Not decided here.
8. **Two forge hosts in one chat** works for git and `glab`; for `gh` it needs the per-host port
   and the wrapper's choice (D3c), and is the first thing the spike measures.
9. **Several accounts on one host**, a person who leaves an organization, and an uninstalled or
   revoked authorization each show as a Notice with a fix.
