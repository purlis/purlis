# Everyone works as themselves, and the forge token never enters a chat

**Proposed 2026-11-20, round 2.** Round 1 (the same pull request, numbered 0127 there; 0127 is the
project-policy ADR on `m75-policy`) proposed a per-owner App and GitLab project tokens. Four
reviewers rejected it: those identities do not serve every collaborator, and they bind a token by
installation, not by the person's access. This round follows the steward's ruling of 2026-11-20.
The facts and their sources are in
[`docs/research/one-login-forge-identity.md`](../research/one-login-forge-identity.md). Nothing here
is built.

## The requirement

Someone creates an organization repo or a personal repo. **Anyone with access to it** (owner,
admin, member, outside collaborator; write; or read-only, with the limits below) signs in to their
own forge account in purlis, clones the repo as a project, and works. The chat runs `gh`, `glab`
and native `git fetch`, `pull` and `push`, opens pull and merge requests, comments, reads CI. The
person types no command, no token and does no setup. github.com, GitHub Enterprise Server (GHES),
gitlab.com and self-hosted GitLab are all in scope.

## Where purlis is: the gap table

BUILT means the code does it today. PARTLY means it does a part, named. MISSING means nothing does.
Code references are at `origin/main` `4568a3c7b4`. Nothing was run against a live forge.

| Action | github.com | GHES | gitlab.com | self-hosted GitLab |
|---|---|---|---|---|
| The person signs in | **BUILT**: device flow, client id only (`forge/registration.rs:7-18`, `forge/device.rs:199`) | **PARTLY**: token paste or CLI import only; the registration "exists on one host only" (`registration.rs:18`) | **BUILT**: PKCE (`forge/pkce.rs`) | **PARTLY**: token paste only (`registration.rs:18`) |
| Sign-in refreshes itself | **PARTLY**: GitHub's sign-in App does not expire tokens, so nothing to refresh (`forge/refresh.rs` serves GitLab) | **MISSING** | **BUILT**: `forge/refresh.rs` | **MISSING** (a PAT does not refresh) |
| A chat's `gh`/`glab` has a credential | **PARTLY**: only through a private App an *owner* registered and installed (`forge/agentidentity.rs:176` `public: false`; `apptoken.rs`). A member who is not an owner gets none | **MISSING** (`apptoken.rs:52-54` `HOST = "github.com"`; #3882) | **PARTLY**: a pasted vault token as `GITLAB_TOKEN` (#7230) | **PARTLY**: the same |
| A chat's native `git fetch`/`pull`/`push` | **MISSING**: since #7241 a locked chat has an empty helper list, an empty ssh agent and no login; purlis's helper answers only the window's git (`forge/gitcredential.rs:12-17`) | **MISSING** | **MISSING** | **MISSING** |
| The brokered `purlis worktree push` | **PARTLY**: works, as the **person's own `gh`/`glab` CLI login**, not their purlis account (`gitbroker.rs:239-257` → `repocmd/sync.rs:147-157` → `forge::helper_for`, `forge.rs:806-823`) | PARTLY, the same | PARTLY, the same | PARTLY, the same |
| A forge API proxy a chat goes through | **MISSING** | **MISSING** | **MISSING** | **MISSING**. The key proxy to reuse (#6762, #6812) is itself only specified: both issues are open and `crates/` has no such module |
| Which chat is asking, from the kernel | **BUILT** for the hook socket (`hookwire/reach.rs`: `WhoAmI`, `Under`, `IN_DOUBT`). **MISSING** for a TCP loopback listener, which has no peer credential | same | same | same |
| Merge, tag, ruleset, release lock | **PARTLY**: the merge hold is the app's (`mergehold.rs`, `floorguard/merge.rs`: text-reading, which Q331 ruled can never be complete); the forge-side lock is #7133, open | PARTLY | PARTLY | PARTLY |
| Confirm a clone as the person's own ("Trust this clone") | **PARTLY**: a trust ask exists in the window (`app/src/TrustAsks.test.tsx`); it does not record an installation | n/a | n/a | n/a |
| Fork a repo you can only read | **PARTLY**: `ForgeBackend::fork_for`, `make_fork`, `open_draft_from_fork` exist (`forge/backend.rs:279-285`, `:708`); nothing hands a chat the way to use them | MISSING | **PARTLY**: `forge/gitlab/making.rs:164` | MISSING |

Roles are not a column because **nothing in purlis reads a role**: every identity above is either
the person's whole login (the brokered push) or an installation or project token a different
person made. Per-user mode is the first design in which the forge decides the role.

## The scenario, host by host

*Maya owns the org `acme`. Ravi is a member with write. Sam is an outside collaborator on Maya's
personal repo `maya/tools`. Lee has read-only access.*

**github.com.** Maya (or any org owner) installs "purlis for agents" on `acme` once. Ravi signs in
by the device flow (code, browser, approve). He clones `acme/app` in the window: purlis finds the
App is not installed on `acme`, and the window offers **Request install**; Maya gets GitHub's
request and approves it once. Until then Ravi's chat can read public data and cannot push. When it is
installed, Ravi's chat runs `git push` and `gh pr create` with nothing typed. Sam signs in, clones
`maya/tools`: the App must be installed on **Maya's account** (Sam cannot install on it); until Maya
installs it, Sam has no push, and the window says whose act is waiting. Lee clones `acme/app`: a
read-only token fetches and pulls; a push is refused by GitHub; the fork route below.

**GHES.** Nothing changes for the person except one registration: someone registers purlis's App on
the host once (any account on that host that may own an App) and gives its client id to purlis. Then
it is github.com's walk, on that host. Until a client id is given, a GHES person who signed in by a
token has the per-user PAT mode below.

**gitlab.com.** Each person signs in (PKCE), and the **first clone** shows one GitLab consent
screen for the separate purlis "agent" application. A Developer chat pushes a branch and opens a
merge request. A Reporter's chat fetches and forks. Nothing is installed on the group, so there is
no owner to wait for.

**Self-hosted GitLab.** The person (no administrator) registers a user-owned OAuth application on
their own GitLab profile, once per host, from values the window shows; the window then runs the
sign-in and the consent screen as on gitlab.com. If the sign-in on that host is a pasted PAT, the
PAT is used only inside the broker (D3), with the PAT's own scopes and expiry, and the person is
told so.

## The decision

### D1. Per-user mode is the base

Every chat acts as **the person who is signed in**, with the person's own access. The forge decides
what the chat may do by the person's role, and purlis narrows further.

**GitHub, github.com.**

- **One public GitHub App "for agents", published by purlis, separate from the sign-in OAuth App.**
  V16c is kept for the sign-in App: it signs in and mints nothing for an agent. The agents App
  signs in too, but *as the user*, and holds no key anyone uses.
- **Device flow with the client id only.** There is no private key and no client secret on any
  device (the device flow's refresh needs no client secret: research 1.4). The App's permissions
  are `contents`, `pull_requests`, `issues`: write; `checks`, `actions`, `statuses`, `metadata`:
  read; **no `workflows`**, no administration, no organization permissions.
- **The token is the person's, 8 hours long.** GitHub reaches only "resources that both the user
  and app can access", in accounts where the App is installed, so the token is *the App's
  permissions ∩ the person's access ∩ the installed repositories*. It is attributed to the person.
  The **app** refreshes it and owns the single-use refresh token, as `forge/refresh.rs` already
  does for GitLab (research 2.3).
- **Install.** An organization owner installs it once per organization. A member or outside
  collaborator **requests**, and GitHub sends the request to the owners (research 1.2). A personal
  repo's owner installs it on their own account; **a collaborator on someone else's personal repo
  cannot install it there**, so the owner must (D4's Notice says so).
- **GHES: one such App per host**, registered once. Its client id is given to purlis in the window
  (Settings › Forge hosts), a Machine-tier value, syncable, that holds nothing secret. **The device
  flow must be ticked by hand in that App's settings**: the manifest flow's documented fields do not
  include it (research 1.1; unverified, see Open items). The window says so, with the page.

**GitLab, gitlab.com.**

- **A separate purlis "agent" OAuth application**, not the sign-in one. Each person grants it once.
  Access tokens last **2 hours** (research 2.3) and the app refreshes them and owns the rotating
  refresh token.
- **A per-chat fine-grained personal access token**, where the host's version supports creating one
  with that credential. GitLab's fine-grained token (19.2 and later) is limited to named projects
  and permissions, is Free on GitLab.com, and acts only as the person (research 2.2). It is created
  by the app at chat start, limited to the chat's projects, **without the `self_rotate` scope**,
  and revoked at chat end. **Probe status: unverified.** The API page says "personal access token"
  for the call and does not say whether an OAuth token is accepted (research G1). Where it is not
  accepted, the OAuth access token is used inside the broker alone and the narrowing is the proxy's
  (D2), not the forge's.

**Self-hosted GitLab.** A user-owned OAuth application per host, made by the person (no
administrator, no instance setting). Where the sign-in is a pasted PAT, **that PAT is used only
inside the broker** and never in a chat. Its scopes and expiry are the person's, and the proxy does
all the narrowing.

**GHES with no App registered** has the same PAT mode.

**Bot mode (opt-in).** Today's Agent identity (an org bot App, per owner) and GitLab **project**
access tokens stay, for a team that wants a bot as the author and forge-side locks (D6). **Group
access tokens are dropped** (one token that spans every project of a group, which round 1's
reviewers rightly refused). Bot mode is a per-project setting, off by default, and the person who
turns it on is the person who can create the identity.

### D2. The token never enters the chat

A chat's process, at most, holds a **sentinel**. The real token stays in the app. Two things reach
the forge for a chat: **a credential helper for git and a forge API proxy**, both app-side.

**Why a helper that answers with the token is not enough.** A credential helper answers whoever
runs it. A chat can run `git credential fill` with `host=github.com` and read the answer. So the
helper in the chat's git configuration answers **only a sentinel** (a fixed username and a per-chat
random string that is worth nothing anywhere else), and the **swap to the real token is made by the
app at the proxy, per request**. That is the key-proxy pattern (#6762, #6812): the harness holds a
stand-in, the proxy removes every credential header the chat sent and puts the real one in.

**How a chat's traffic reaches the proxy.**

- **Git:** the chat's per-chat git configuration, passed in the environment as #7241 does,
  rewrites each of the chat's own remotes (`url.<loopback>.insteadOf`, for the HTTPS, `ssh://` and
  scp forms) to the proxy's loopback address. The clone's config on disk is not touched, and the
  ssh agent stays shut.
- **`gh` and `glab`:** `GH_TOKEN`/`GH_ENTERPRISE_TOKEN` and `GITLAB_TOKEN` hold the sentinel, with
  `GH_HOST`/`GITLAB_HOST` as today. **How their requests reach the proxy is unproven** (Open item 1):
  github.com has no API base-URL variable; the candidates are a per-chat configuration file with
  an HTTP unix socket (purlis owns the empty `GH_CONFIG_DIR`/`GLAB_CONFIG_DIR` since #7241), or a
  proxy with a per-chat CA, which Go's macOS client does not read from `SSL_CERT_FILE`. A spike
  decides before a line of the build.

**Who is asking is the kernel's to say, never the chat's.** The proxy finds the connecting process
by the kernel and asks `WhoAmI`'s rule (`hookwire/reach.rs`: ancestry, the app above, the chat's
own record) which chat it is in. **A value the chat supplies decides nothing**: not the sentinel,
not a header, not the port. A unix socket gives a peer credential; a TCP loopback socket does not,
so the connecting process is read from the kernel's socket table, and **the answer must agree with
the per-chat listener the chat was given**. Where the proxy cannot tell, it refuses (`IN_DOUBT`).

**What the proxy answers.**

- **Match exactly** on scheme, host, port and **normalised path** against the chat's own repos: the
  clones the person confirmed (D4). Percent-encoding is decoded once, `.` and `..` segments and
  `//` are refused, and case is folded where the forge folds it. **No prefix match**:
  `acme/app` is not `acme/app-secrets` and not `acme/app.git/../x`.
- **Refused at the proxy, whatever the token could do:** merge (REST and GraphQL, including
  auto-merge); ruleset, branch-protection and default-branch changes; tag and release writes;
  creating, moving or deleting the default or a protected branch; repository, hook, secret and
  deploy-key writes. The rule is **an allowlist of write operations** (REST routes; GraphQL
  parsed, every mutation not on the list refused), not a list of bad ones, since Q331 found that a
  list is never complete.
- **Git over HTTP:** the proxy reads the ref commands that start a push and refuses the default
  branch, a protected branch, every tag, and any ref the person's own role does not allow. It
  cannot see paths from them (see Findings, CI files).
- **Every answer is audited** with the chat's id, the repo, the endpoint and the verdict, app-side.

**The brokered push** (`purlis worktree push`, the app's own act) uses the same token and is not a
route through the proxy. It pushes to **the address recorded for the clone, passed to git
explicitly**, never to whatever `origin` resolves to now (a chat can rewrite a remote). It
stops reading the person's `gh` or `glab` login (the gap table's PARTLY).

**Revocation per chat is moot**: the token never leaves purlis. It stops being injected when the
chat ends. A per-chat GitLab PAT (D1), where used, is deleted then.

### D3. The token is only ever the person's, and only in the app

- The sign-in token, the agent token and any PAT are keyring items of the app
  (`purlis/@forge/…`, as ADR 0077 §7), unreachable by an approval and never in an environment, a
  file, an argument or an error.
- A pasted PAT or a signed-in token is read by the broker alone.
- A refresh is the app's alone (one process may rotate a single-use refresh token, ADR 0077 §3).
  The proxy and the helper read what is stored and never refresh.

### D4. An installation is trusted only as the person confirmed it

A chat's token is injected only for a repository **the person cloned from the window, or told
purlis to trust ("Trust this clone", for a clone that already exists)**, as ADR 0077 amended
2026-10-29 E3 builds the list. An installation of the App on any other repository (an attacker who
installs it on a dependency the person pulls, an organization that adds it without the person's
knowledge) **gets nothing injected**, and the proxy refuses it by the exact-match rule. The window
records the confirmation: the clone, the host, the repo **id** (not only the name, so a rename or a
transfer to another owner does not carry the confirmation), and the App's **id**.

A clone made from the CLI is outside the list (#7196), and stays so; the window offers "Trust this
clone" and says which clones are untrusted.

### D5. Provisioning comes only from the person's press in the window

Sign-in, **Request install**, the OAuth grant and registering a host's App happen **only on the
person's press in the window**, never from a chat, the CLI or a socket call. These run on the
`local-ui` scope and refuse inside a chat's tree, as `sign_in_with_grant` already does
(`forge/accounts.rs`, a `HostScope`, V22c). **It triggers at clone**, and at "Trust this clone" for
an existing clone, not only at sign-in: a clone on a repo without the App shows the next press
there. A window that finds an installation missing says whose act is needed and sends no one a
request without the press.

### D6. Locks

- **Bot mode keeps #7133's forge-side lock.** The bot's installation can be restricted by the
  forge's rulesets and protected branches to the app's own identity.
- **Per-user mode relies on the proxy's refusals plus the merge hold** (`mergehold.rs`: the app
  merges, on the person's press), and **the forge cannot enforce more there**. Plainly: the chat's
  token is the person's. Where the person is an administrator, GitHub's branch protection does not
  stop their token unless the rule applies to administrators, and nothing purlis does changes
  that. Anything not through the proxy (the person's own terminal) has the person's whole access.
  The forge-side backstop for a chat is the per-chat fine-grained PAT where it exists (D1), and
  GitHub's App permissions (no `workflows`, no administration).
- **Nothing in this record asks anyone to add anything to a bypass list.**

### D7. The roles

The chat has **exactly the person's role**: not more (the token is the intersection) and not less
than the proxy's list allows.

| Role | Fetch, pull | Push a branch | PR/MR, comment | CI read | Merge |
|---|---|---|---|---|---|
| Owner, admin, maintainer | yes | yes (not the default or a protected branch) | yes | yes | the person's press only |
| Write, Developer, member | yes | yes (the same) | yes | yes | the person's press only |
| Outside collaborator with write | yes, where the App is installed on the owner | yes (the same) | yes | yes | the person's press only |
| Read-only, Reporter | yes | **no** (the forge refuses) | comment on issues, yes; open a PR from a **fork**, below | yes | no |

**Forks.** For a read-only person, `purlis` offers **Make my fork** in the window (their press, D5):
the app creates a fork under the person's own account (`make_fork`, `forge/backend.rs:285`), adds it
as a second trusted repo of the same chat, and the chat pushes there and opens the pull or merge
request across (`open_draft_from_fork`, `:708`). **Supported on github.com and gitlab.com.** On GHES
and self-hosted GitLab it is **supported where the host allows forks for the person**, and a host or
organization that forbids them (GitHub lets an owner disable forking of private repositories; a
GitLab group can too) leaves the person read-only, said in a Notice. A fork needs the App installed
on the person's own account (a GitHub press) and a GitLab grant: both are the person's own acts.

## What the round-1 reviewers found, and how it closes

| # | Finding | How it closes |
|---|---|---|
| 1 | **GitLab self-rotation**: a token with `self_rotate` rotates itself and the copy a chat holds becomes a new one | **Moot**: no GitLab token is in a chat (D2). A per-chat PAT is created **without** `self_rotate` (D1). |
| 2 | **CI files**: a chat that can push can change CI and run code with the token's secrets | **GitHub:** the App has no `workflows` permission, so GitHub rejects a push that touches `.github/workflows`. **GitLab:** a push of `.gitlab-ci.yml` (or an included file) **runs a pipeline as the person**, with that person's access to CI variables on that branch. Stated, not hidden. The proxy **refuses** such a push by unpacking the pushed pack in a quarantine and reading its changed paths (the app has `diffscan.rs`); a protected variable still needs a protected branch the chat cannot reach. **Designed, not built, and it costs a buffered pack (Open item 4).** |
| 3 | **Audit**: the forge's log says only "the person" | Every answered request is audited app-side per chat (D2), with the chat's id and the endpoint. The forge cannot tell chats from the person, and this says so (D6). |
| 4 | **Offboarding**: a bot's token outlives a member who left | A user token **dies with the person's membership**: the intersection with their access shrinks at once (D1). Bot mode keeps the old risk and says so in its Notice. |
| 5 | **Lookalike App names**: an App named like purlis's is installed | The window shows the App's **id** beside its name and trusts only the one recorded id (D4). |
| 6 | **Installation tokens are bound by installation, not by the person** | The user token is the App ∩ the person's access ∩ installed repos (D1), plus the exact-match rule (D2). |
| 7 | **The App does not serve every collaborator** (an org member who is not an owner, an outside collaborator, a read-only person) | Per-user mode: each person has their own token and role (D1, D7); install is the owner's one press, and a request path exists (D1). |
| 8 | **Group tokens span every project** | Dropped (D1). |
| 9 | **A helper answers anyone who runs it** (`git credential fill`) | The helper answers a sentinel (D2). |
| 10 | **A remote can be rewritten** and the brokered push follow it | The brokered push is passed the recorded address (D2). |
| 11 | **An attacker installs the App on a dependency** | No token is injected for a repo the person did not confirm (D4). |
| 12 | **Provisioning from a chat** | Only on the person's press (D5). |
| 13 | **Merge, ruleset and release-write through an allowed token** | An allowlist at the proxy, refusals for all of them (D2); the merge hold stays (D6). |

## What it supersedes or amends

- **ADR 0077, V16c, wording.** *"A GitHub App the organisation registers itself"* becomes *"for a
  team that opts into bot mode"*. The sign-in App stays sign-in only (its client id, no key, no
  secret): amended as far as that wording and no further. The amendment is a file under
  `docs/adr/0077/`.
- **ADR 0077, FI3.** Unchanged and now enforced harder: the person's token is **the** agent's
  token, and it never reaches the agent (D2). FI3's "agents keep their own narrow identity"
  becomes "agents act as the person through the app, never with the person's token in hand".
- **ADR 0070, amendment 2026-10-12-02 (#752).** Its "never inside a chat's tree" stands for the
  person's helper. The chat's helper answers a sentinel only.
- **ADR 0067's credentials class.** Unchanged: it denies the person's login to a chat; this record
  gives the chat a route that holds none.
- **#1351** closes by D2 (the helper does not read the person's keychain). **#7133** is bot mode's
  lock. **#2053** SD-7a/b is bot mode. **#3882** is GHES's half of D1. **#7196** and **#7210** keep
  their asks. **#7215**'s "Free takes a Developer user's token" is per-user mode.
- **The round-1 `agent-identity-gitlab-` store** is not made. Its `**Tier:**` line is not needed.

## Consequences

- **A person signs in and presses once per owner**, and every locked chat in the repos they
  confirmed can push, pull, open pull requests and read CI as them. There is no `gh auth login`
  and no pasted token.
- **A request waits on an owner** where the person is not one (GitHub), and says whose act.
  **A personal repo needs its owner to install the App** for a collaborator. No design removes this.
- **The app is on the path of every forge call a chat makes.** If it is gone, a chat's forge calls
  fail with one sentence. Offline git is unchanged.
- **A new store: the GHES client id per host** (Machine tier, syncable, nothing secret), which
  goes in `docs/plane-format.md` with a **Tier:** line first, so
  `every_store_the_plane_format_names_has_a_tier` stays green. The tokens are keyring items under
  `purlis/@forge/…`, as ADR 0077 names. The per-chat sentinel is memory only.
- **purlis publishes and operates two public registrations**: the GitHub agents App, and the GitLab
  agent application. Their client ids ship in a release (as `registration.rs` does).

## Open items (nothing here is proven)

1. **How `gh` and `glab` reach the proxy** (D2). A spike, with a real `gh` and `glab` on macOS and
   Linux, comes before any ticket.
2. **Kernel identity on a TCP loopback socket** (D2): the socket-table read for macOS and Linux is
   not in `purlis-same-user` today. The alternative, a unix socket, has no way into git's
   configuration. Refuse when in doubt.
3. **GitLab: whether the fine-grained PAT call accepts an OAuth token** (research G1), and which
   versions have it. **GitHub: whether a manifest can switch the device flow on** (read as not,
   research 1.1). Both need one live probe.
4. **The CI-file refusal on GitLab** needs the proxy to buffer and unpack a pack: a size limit, and
   a cost in latency, to be set.
5. **SAML single sign-on organizations** and **the App on enterprise-owned organizations**: not
   read; whether a user token needs a per-organization authorisation is unchecked.
6. **A GraphQL operation allowlist** is a list someone keeps. The review that builds it should
   reread Q331.
7. **Pools and runners** (ADR 0077 E4): a runner without its desktop has no person to refresh a
   token. Not changed here.
8. **Several accounts on a host**, a person who leaves an organization, and an App the owner
   uninstalls each show as a Notice with a fix (Settings › Agent identity), never as a failed push.
