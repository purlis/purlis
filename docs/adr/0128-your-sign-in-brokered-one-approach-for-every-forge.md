# Your sign-in, brokered: one approach for every forge

**Proposed 2026-11-20; brought in line with what was built, round 10, 2026-11-21.** Round 1 (numbered 0127 there; 0127 is the project-policy ADR on
`m75-policy`) proposed a per-owner App and GitLab project tokens, and four reviewers rejected it.
Round 2 proposed per-user tokens as the base and a bot as an opt-in second mode; the person
rejected two modes. Round 3 was one approach. Three reviewers (security, feasibility, coverage)
read round 3 and said **fixes needed, the shape stands**; round 4 folded the steward's rulings on
their findings. Two reviewers read round 4 (Fable round 3) and said **fixes needed, the shape
stands**; **this round folds the steward's fifteen rulings on those findings**. Where a ruling made
a claim the record had to check, what was checked and what was only read from documentation is said
where the claim is made. The facts and their sources are in
[`docs/research/one-login-forge-identity.md`](../research/one-login-forge-identity.md), sections 7
and 8. Nothing was run against a live forge. What was read, what was tried and **what was built** is
said where each claim is made. **Slices A (proxy core), C (credential sources), D
(readiness) and E (the `gh`/`glab`/`git` spike) build against this record; its section numbers
(D1 to D10) are the ones they were given.**

**Round 8 adopts spike E's results** (PR #7270, `tests/one-login-spike/spike.mjs`: a fake forge and
a stand-in proxy, a real `gh`, `glab`, `git`, `cargo`, `npm` and `pip`, on Linux and macOS CI, 66
cases each, **no real forge and no real token**). It **rewrites D3b and D3c**, changes D3a's "not
a TLS interceptor" (scoped, with its security consequence named in D3h.10), amends D3g and D3i,
and moves the requirement row for `gh`/`glab`/`git` from "conditional" to "condition met on both
systems for the mechanisms below, with the gates that remain named". **What the spike proved is
marked *(spike E)*; what it did not is marked *not proven* and stays a gate.**

**Round 10 brings the record in line with the one-login train** (slices A to K, each reviewed;
the train lands on `main` as one pull request, with `forge-wiring` on by default). Where a round-9
sentence said what would be built, it now says what is built, and where the review of a slice moved
a decision, the decision says so and names the slice (A2 #7293, B #7292, F and F2 #7299, G #7288, H
#7289, J #7295; follow-ups in #7301). Six things changed in substance:

1. **GraphQL is answered per type, not refused at `node`, `nodes` and `search`** (D3e): a field
   allowlist per type, ids tied to the chat's repos, one-repo `search`, introspection, and a related
   repository shown as its name only. REST reads under a repo are a pattern allowlist too (D3d).
2. **A CI-file push is decided by path, never by parsing** (D8.3). The tree diff of D3f and D8.3 is
   read in an app-owned quarantine; what a pipeline includes decides nothing.
3. **The egress denial is built by name, by the name inside a tunnel (SNI) and by address** (D3g),
   and its limits are named (D3h.11 to D3h.14).
4. **GitHub's second token fails closed on every doubt** (D1.1, D4): unknown scopes serve no chat.
5. **A route belongs to a run** (D3a, J): a chat reattached after an app restart has no route, and
   a runner's chat has none yet (D9).
6. **Migration is built** (H): the old routes are retired behind the same switch, and a vault
   forge token is refused to a chat (Migration).

**What the reviews accepted as residuals is listed in one place: D3h.6 to D3h.20.** The person-facing
account of all of it is [`docs/forges.md`](../forges.md).

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
**Built (G, #7288).** The second token is a second device sign-in on the person's press
(`accounts::workflow`), kept as its own keyring item and read only by `for_ci_push`, for a push the
person approved; no chat's route reads it, and a source scan fails if any code but the app names
it. GitHub may fold a re-consent into the existing grant, so the sign-in **reads the everyday
token's scopes back** (`X-OAuth-Scopes`) and keeps the second token only if the everyday one still
lacks `workflow`. **A read-back that says nothing of scopes, fails, or shows `workflow` on the
everyday token keeps nothing**, and the push is not offered: the ask says to push it from the
person's own terminal, rather than the everyday token gaining `workflow`. **Every doubt fails
closed:** `workflow::begin` marks the account's scopes **unknown before the flow starts**, and only
a clean read-back writes them, so a cancelled flow, an app that quit and a failed read-back each
leave the account unknown, **and an account whose scopes are unknown, or whose everyday token holds
`workflow`, is served to no chat route** (a Notice says why and what fixes it: a sign-in without
`workflow`, or a read-back that succeeds). A fine-grained or pasted token returns no scopes header
and so counts as unknown the same way. GitLab has no second token: its one `api` scope covers CI
files, and the hold is the protection (D8.3). **Accepted residual (#7301):** GitHub's device code
stays approvable until it expires after a cancelled flow, so approving the old code afterwards could
fold `workflow` in; the account stays unknown until a read-back, and the follow-up records "flow
pending until expiry" so the window says so.

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
chat), not a second port. It speaks to the chat three ways, all the chat's own: **plain HTTP over
a per-chat unix socket** (`gh`, D3c), **`CONNECT` on the chat's loopback port, with TLS terminated
under a per-chat purlis CA for the recorded forge hosts only** (`git` and `glab`: D3c), and the
existing tunnel for every other host, host and port only, **never opened up**. **The
download-only hosts (`codeload.github.com`, `release-assets.githubusercontent.com`,
`objects.githubusercontent.com`) are tunnelled, never terminated** (a plain allowed `CONNECT`
with their real certificates: D3i). **It speaks TLS to the forge with the system's roots, a strict
chain and hostname check and no fallback**; the upstream client **ignores the chat's environment
and never trusts a per-chat CA**. **This changes round 3's "no TLS
interception", and the change is scoped**: only the forge hosts of D3g and D3i's closed list, only
for a client the chat's environment told to trust that CA, **never added to the system or the
keychain trust**; the security consequence is D3h.10. **An egress proxy held elsewhere** (a session host's,
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
  port, and that chat's own unix socket, for that chat**, listed. **The sandbox profile gains one
  allowed unix socket per chat (its own), and no other** *(spike E: `gh` connects to it; the
  socket must be in the chat's allowed unix sockets)*.
- **Item 5** (*"host and port only … no path rules"*): **amended for the forge route only.** A
  tunnel (`CONNECT`) to any host the proxy does not terminate is still host and port only. A
  request on the forge route (**plain HTTP over the chat's unix socket, or inside a `CONNECT` to a
  recorded forge host that the proxy terminates**) is read: method, canonical path, headers and, for
  GraphQL and a push, the body.
- **Item 7** (*"no buffer past a fixed size"*): the forge route **streams** (clone and fetch packs,
  release assets, artifacts) with a bounded window; **the one exception is a push whose changed
  paths are read (D8.3)**, which is held up to a size limit (Open item 5).
- **`egress.rs`'s refusal of every `Transfer-Encoding` head** (a request smuggling guard in the
  built proxy): **the forge route accepts `Transfer-Encoding: chunked` on a request body**, because
  `git push` over HTTP sends a pack of unknown length that way, and so can the anonymous route's
  upload-pack `POST` (D3i), which gets the same de-chunking. This is **new code in `egress.rs`**:
  the route de-chunks the body itself (it is read for D3f and D8.3 anyway), forwards a
  `Content-Length` request upstream, and still **refuses `Content-Length` together with
  `Transfer-Encoding`, any `Transfer-Encoding` value but a single `chunked`, and a chunked body on
  every route but the forge's and the anonymous read route's** (D3g, D3i). The tunnel and every non-forge request keep the built
  refusal.
- **2026-10-10-02 item 4** (*no other local port*; `allowLocalBinding` false): unchanged, and
  D3h.2 says what a managed setting that turns it on costs.

**Reused (built):** ADR 0067's per-chat egress proxy and its sandbox profile (amended 2026-10-10,
#1664), which allows a chat's network traffic only to its own proxy; `forge/refresh.rs`,
`device.rs`, `pkce.rs`; `mergehold.rs`; the keyring items of ADR 0077 §7 and the recorded clone
list of ADR 0077 amended 2026-10-29 (E3: *"github.com remotes at chat start"*; **ADR 0128
supersedes that amendment (below) but keeps E3's meaning, a chat's repos are the clones the person
recorded; D4's list, its confirmation and its numeric ids are new code**). **`diffscan.rs` is not
reused**: it is a pre-commit scanner that reads a commit's index, and D8.3's pack and tree-diff
reading is new code (at most its path patterns are reused). **Not built:** the key proxy of #6762 and
#6812 is specified and has no code in `crates/` at `4568a3c7b4`. **This record's proxy is new
code**: the reverse proxy, the credential injection (it removes every credential header the chat
sent and adds the person's), the canonical-path allowlist (D3d), the GraphQL resolver (D3e), the
git-protocol checks (D3f), the response rewrite and the audit (chat id, repo, method, canonical
path, verdict).

**What is built (A2 #7293, B #7292, J #7295).** The listener is per chat: **a unix socket for `gh`**
(its folder `0700` and the app's, the socket `0600` bound with the umask held, a lax, foreign or
linked folder refused, and a peer of another user closed unread) and **the `CONNECT` door for `git`
and `glab`**. A recorded host is terminated under a **per-chat authority**: the key is in memory
only; the certificate carries **critical name constraints for the chat's hosts with every address
excluded**, a path length of 0, and leaves for those hosts only; the SNI must be the `CONNECT`
host. The upstream is strict TLS with the system roots, no proxy, no redirect and no idle
connection. **One request per connection**, strict framing, a total deadline per request and
per-piece progress on streams. **Answers are read before the chat gets them (the scrub):** the real
token (raw, percent-encoded and Base64, at three alignments of the Basic pair) ends the answer;
credentialed clone URLs, userinfo in any of its spellings (JSON-escaped, HTML entities, scheme-less,
`git://`) are cut down at every byte boundary; `Location` and `Link` are rewritten; a redirect is
followed once, anonymously, and only to a download host on `gh`'s socket. Request bodies are
**held**, not streamed (JSON up to 1 MiB, a push up to 32 MiB; a chunked push is sent with its
length); responses stream. **A route belongs to a run, not a chat** (J): a chat with recorded repos
gets, per run, its own proxy, sentinel, `0700` folder (`pf-<uid>/<pid>-<hex>/`, refused unless a
real directory this user owns) and socket. It stops with the session's close, its program's exit,
its project closing or the app's exit, and **a restart's old run ending never touches the new run's
route**. The per-chat wiring a chat is given (its `gh` environment, config folder and wrapper, git's
credential pairs with the helper reset first, `glab`'s environment) is written by
`forgewire::compose` into the app's own state, never the chat's folder, new and held to no link. The
ambient forge tokens are scrubbed from the chat's environment by **one list**
(`forgewire::is_ambient_forge_token`: `GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`,
`GITHUB_ENTERPRISE_TOKEN`, `GITLAB_TOKEN`, `GITLAB_ACCESS_TOKEN`, `GLAB_TOKEN`, `OAUTH_TOKEN`,
`CI_JOB_TOKEN`, the `PURLIS_FORGE_TOKEN*` and `CHARTER_FORGE_TOKEN*` prefixes and every connection
variable, compared without regard to case), then the three the tools read are set to the sentinel.
**What is not built:** a **reattached chat** (after an app restart) and **a chat that outlives the
app** have no route (their forge calls keep the token connections or fail, never an earlier app's
route; the next app's sweep removes the folders); a **runner's chat** is not wired (D9); `gh` through
a GHES alias is mapped and **not proven** by a real `gh`.

### D3b. The chat's own port, and who is asking

**The forge route lives on the chat's own per-chat HTTP egress port** (ADR 0067 amended
2026-10-10-01 item 1: one pair of ports per chat, opened when it starts and closed when it ends).
Round 4's "one port for every chat on the machine" is **withdrawn**: ADR 0067's rule is that the
port is the chat, and a shared port would have needed the chat to say who it is. The route
extends that port to **a `CONNECT` the proxy terminates for a recorded forge host** and to
**streaming** (D3a); `gh` uses **the chat's own unix socket** beside it (D3c).

**Identity is that port, or that socket's path, plus a per-chat secret.** The port or the
socket path names the chat (**only that chat's sandbox may connect to it**). **The socket's
directory is mode 0700 and the socket 0600, owned by the app and not writable by the chat.** A
process of the same user outside any sandbox can still connect, so **the sentinel is the
barrier, not the path**; **where the platform gives a socket's peer credentials, the proxy reads
them as one more check, and never as the identity**. The secret, called
**the sentinel** where a tool sends it, is the second factor that makes a process that merely
reaches the port (the harness, a hook: D3h.1) not enough:

- **The secret is at least 128 bits from the operating system's random source**, minted **at every
  chat start and at a resume** and never reused, held in the app's memory and the chat's
  environment only, and worth nothing anywhere else.
- **It is compared in constant time.** **Attempts are limited**: a port **or socket** that receives more than a
  few wrong secrets in a minute answers nothing for the next minute and raises one Notice naming
  the chat. A wrong secret is refused as IN_DOUBT and audited.
- **Each tool carries it where it already sends a credential**, and the proxy reads it from only
  these places, **strips them all** and injects the person's (D3a). **Each tool sends it as its
  own credential** *(spike E: each reached the proxy as below on both systems; the proxy swapped
  it for the real token, decoding Basic first, and the forge saw the real token and never the
  sentinel)*: git, as the **password of Basic `Authorization`**, answered by the credential
  helper (`username=x-access-token`); `gh`, as `Authorization: token …` from `GH_TOKEN` and
  `GH_ENTERPRISE_TOKEN`; `glab`, as `GITLAB_TOKEN`. Rounds 4 to 7 put git's in
  `Proxy-Authorization` through `http.extraHeader`; **that is replaced** by the helper, so git
  needs no header configuration.
- **The host is the request's own, and it is checked, never trusted.** With the rewrite gone
  (D3c) a request names its forge by **the host it was sent to**: `github.localhost`'s API host
  for `gh`, the `CONNECT` target for git and `glab`. The proxy requires that host to be **one of
  the host names this chat's recorded repos are on** (D4), and **builds the upstream from its own
  recorded value, not from the request's text**: a request whose `Host`, `X-Forwarded-Host`,
  `Forwarded`, `X-Original-URL` or similar header disagrees with the host the connection was
  made to is refused. **No path segment chooses an upstream.** **The route rule is one statement: a request is
  credentialed if and only if its repo is recorded AND it carries a valid sentinel.** Any other
  request goes on the anonymous route with **every `Authorization` stripped, the sentinel
  included**, and carries no credential: D3i.)

Why the identity is not a kernel read of the connecting process (a TCP loopback socket has no
peer credential; `git` and `glab` do not speak a unix socket, though `gh` does and uses one) is
round 3's weighing, unchanged (research 7.3). **Round 9 corrects its reach:** `gh`'s unix socket
does have peer credentials where the platform offers them, and the proxy uses them as an extra
check on that socket only; they say which user connected, not which process, so they never
replace the sentinel.

**The one exception: a chat the person chose to run with no sandbox** (ADR 0124's No sandbox; the
person decided to run it unconfined). Such a chat **gets the same proxy route and the same
environment**, so forge works in it exactly as in any chat. What purlis **cannot** do there is stop
the chat's own use of the person's login: it can read `~/.config/gh`, the keychain and the git
credential helper as the person, and there is no egress rule to deny the forge hosts (D3g).
**D6 and the point-by-point table say this plainly**: at No sandbox the proxy is the route purlis
provides and not a guard, and the hold on merges and tags is, as ADR 0124 §7.3 says, to catch
accidents only. This replaces round 4's "an unsandboxed chat gets no proxy route".

### D3c. How each tool is pointed at it

A chat's git configuration is passed in the environment, as #7241 does, and **the person's**
clone's `.git/config` is not touched. **One carve-out, headless only (D9):** a CI checkout leaves
its own authentication (`actions/checkout`'s `http.<host>.extraheader` token) in the clone's
`.git/config` on disk, where a chat could read it, so **the runner removes that entry, and any
include that carries it, from the clone's config before a chat starts** (D9). It is the CI's
credential in a checkout the runner owns, not the person's configuration.

Every tool's route is **to the chat's own port or socket** (D3b), set **in the environment** and
**in the chat's own config directory**, and each is spike E's, on both systems *(spike E)*.
**One per-chat CA** (a key held in the app's memory only, and a certificate written to the chat's
folder) serves every `CONNECT` the proxy terminates (D3a). It is **named in the tool's own
variable, never in the system or the keychain trust.**

- **git: the environment alone** *(spike E: `git ls-remote` and a full `git clone` reached the
  forge with the real token on both systems)*. `HTTPS_PROXY` is the chat's port, and the per-chat CA is named **per
  recorded forge host, as `http.https://<recorded-host>/.sslCAInfo` through `GIT_CONFIG_*`**
  (**never `GIT_SSL_CAINFO`, chat-wide**: it replaces git's whole bundle, so every other host
  would fail its certificate check). **A credential helper, scoped to the recorded forge
  hosts, answers the sentinel** (`username=x-access-token`, `password=<sentinel>`; purlis's own
  small command, holding no token, **reading the sentinel from the environment and doing nothing
  on `store` and `erase`**), all through `GIT_CONFIG_*` as #7241 does. **`git remote -v`
  is unchanged (`github.com`), and there is no `insteadOf`, no `/anon/` prefix and no change to the
  person's clone's `.git/config`.** The proxy decides the route from the repo in the request and the sentinel
  (D3b): **a recorded repo with a valid sentinel takes the credentialed route, and every other
  request takes the anonymous one with no credential** (D3i), which replaces round 7's
  longest-prefix rewrite. **A test requires that #7241's helper reset (`credential.helper=` first)
  precedes purlis's helper**, so `osxkeychain` cannot store the sentinel over the person's entry. The
  helper list is **purlis's one helper, not empty**: D3g says what that changes.
- **gh: `GH_HOST=github.localhost` and a unix socket** *(spike E: `gh api user`, `gh api graphql`,
  `gh api repos/o/r` and `gh repo view o/r --json name,url` reached the proxy with
  `Authorization: token <sentinel>`, plain HTTP over the socket, on both systems; no CA, port or
  DNS)*. Per chat, a **`GH_CONFIG_DIR`** (holding no login, only the `http_unix_socket` setting; D3g) whose `config.yml` sets
  **`http_unix_socket`** to the chat's socket, `GH_HOST=github.localhost`, and
  `GH_TOKEN` and `GH_ENTERPRISE_TOKEN` the sentinel. **The socket's path is the chat's identity:
  only that chat's sandbox may connect to it (D3a), plus the sentinel (D3b).** **Two GitHub-shaped
  hosts (github.com and a GHES host) share `github.localhost`, so a call with no repo (`gh api
  user`, `/graphql` without one) fails closed unless exactly one GitHub host is recorded for the
  chat, and one host's token is never sent to the other.** Paths are
  github.com's shape (`/user`, `/graphql`), not `/api/v3`, so the proxy builds the upstream from
  its own recorded host (D3b). **`NO_PROXY` no longer matters to `gh`**: the round-7 wrapper that
  cleared it is dropped (*spike E: with the chat's own `NO_PROXY=localhost,…`, `gh` over
  `HTTP_PROXY` bypassed the proxy and failed on macOS*; the socket does not use it).
  **Remote matching, ruled:** with `GH_HOST=github.localhost` and a clone whose remote is
  `github.com`, `gh pr view`, `pr checks` and `pr create` fail before any request with *"none of
  the git remotes configured for this repository correspond to the GH_HOST environment
  variable"* on both systems *(spike E)*. **The gh wrapper purlis writes for each chat therefore
  passes `-R github.localhost/<owner>/<repo>`**, from the recorded repo of the working directory,
  to every subcommand that takes it and when the chat gave no `-R` of its own *(spike E: `gh pr view
  1 -R github.localhost/o/r` reached the proxy with the remote left at `github.com`)*. **Not an
  alias of the remote**: that needs a rewrite of the clone's config or an `insteadOf`, which D3c
  has just dropped. **What it gives up:** `gh`'s inference of the repo from the remotes, and
  `-R` is a per-call flag that **needs the pull request argument**, so a bare `gh pr view` or
  `gh pr status` of the current branch is not offered (the chat passes the number or the branch);
  a chat that works across repos passes its own `-R`. **The wrapper never reads `gh`'s output**
  (a table of the subcommands that take `-R` is the whole mechanism), and **the sentence above
  must not reach the person**: if it appears, a subcommand is missing from the table, which is a
  test failure, not a message to show. `GH_REPO` was **not run** and is not relied on. For GHES,
  the virtual host maps to `https://<host>/api/v3/` and `/api/graphql` at the proxy: **not
  proven** *(spike E ran only a github.com-shaped fake)*.
- **glab: `HTTPS_PROXY`, `GLAB_CA_CERT` and `GITLAB_TOKEN`** *(spike E: `glab api user` and
  `glab api projects/o%2Fr` reached the forge with the real token on both systems;
  `GLAB_CA_CERT` is glab's own setting and was honoured on macOS too)*. `HTTPS_PROXY` is the
  chat's port, `GLAB_CA_CERT` the per-chat CA, `GITLAB_TOKEN` the sentinel, and **`GITLAB_HOST`
  stays the real host**. **Chosen over `GITLAB_HOST=127.0.0.1:<port>` with `GLAB_API_PROTOCOL=http`**
  (which also worked, with no CA), because the real host keeps `glab`'s own repo inference from
  the remotes and the one CA and one `CONNECT` mechanism git already uses, so there is no second
  port-per-host route or a plain-HTTP mode to keep, and `glab`'s remote matching never has a host
  that disagrees with the remote. **The `GITLAB_API_HOST` and `GLAB_API_PROTOCOL` settings of
  rounds 4 to 7 are dropped.**
- **cargo.** **`net.git-fetch-with-cli=true` is set for every chat** (`CARGO_NET_GIT_FETCH_WITH_CLI`),
  so cargo's git dependencies go through `git` and the route above *(spike E: works on both
  systems)*. **libgit2, cargo's default, is a named gap: it got 401 on both systems in every
  variant tried** (including a single-entry helper config in `$HOME/.gitconfig`), cause unknown.
  A chat that unsets the setting, or a tool that links libgit2 itself, gets the Notice (one
  sentence, with *"Set `net.git-fetch-with-cli`"* as its fix). Cargo also asks `api.github.com`
  for `GET /repos/o/r/commits/HEAD` **with no credential** before it falls back to git: the proxy
  sees an unauthenticated API call and serves it on the anonymous route (D3i).
- **npm and pip: tarballs tunnelled, never terminated** *(spike E)*: npm
  `install https://codeload.github.com/o/r/tar.gz/<ref>` and pip through `HTTPS_PROXY` reached
  the forge and installed on both systems. **Round 9: the download-only hosts are a plain allowed
  `CONNECT` with their real certificates (D3i), so purlis sets neither `NODE_EXTRA_CA_CERTS` nor
  `PIP_CERT`**, and these tools carry no credential. That the spike's npm line set an `_authToken`
  and its pip URL carried user info is the spike's fake forge's way of checking the swap, and
  **purlis sets neither**.
- **The proxy re-emits canonical URLs.** It never forwards the chat's own request line: it parses,
  validates against D3d and builds the upstream URL itself. `Link` response headers are rewritten
  to the chat-side address, or `--paginate` would leave the proxy.
- **Pinned versions** *(spike E ran: Linux `gh` 2.103.3, `glab` 1.119.0 and `git` 2.56.1; macOS
  `gh` 2.105.0, `glab` 1.131.0 and `git` 2.56.1)*. purlis records the proven versions **per
  system and tool in code beside `forgelock::PROBED`**, as the probe records a route, and **a chat
  whose `gh`, `glab` or `git` is another version gets the Notice** (the version it found, the
  one proven, the fix: install it, or Dismiss). **The route is still set up**; the Notice says it
  is not one purlis has run. `GH_HOST=github.localhost` and `http_unix_socket` are
  **undocumented or lightly documented**, so a `gh` bump re-runs the spike's job before the list
  changes. (Round 7's `gh` 2.69.0 was a reading, not a run, and is withdrawn.)
- **Not proven, and still gates** *(spike E)*: **`gh repo clone`** (by `o/r` and by URL: the fake
  forge's answer did not satisfy `gh`'s `RepositoryInfo` query, so what it writes to `git remote
  -v` is not recorded), **`gh pr create`, `pr checks` and `pr checkout` end to end** (only
  remote matching and reach are proven; the fake has no PR endpoints), a **paginated list** and
  **`run view`**, **a real github.com or gitlab.com** (everything above is a fake on loopback; the
  proxy was a stand-in; no real token was used), and **GHES**. Those need a fuller fake or the
  real proxy, and are **slice A's and the live check's** (Open item 1). **If `gh` fails them,
  the gap is named as `gh`'s, and `git` and `glab` still ship**; no shim that reimplements the
  API and no token in an environment is acceptable. **No system-wide CA is acceptable.**

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

**Built (A #7275, A2 #7293): the reads under pulls, issues and commits (GitHub) and merge requests
and issues (GitLab) are a pattern allowlist, and the query is too.** A read is served only on a route
the list names, and **only with query keys the list names** (one global list of keys for the listed
routes, a residual: D3h.15); `sudo` is refused on every read and the `Sudo` header is stripped;
`render_html` and `include_html_description` are refused on every read. **Refused by name, with a test
per forge:** the timeline, sub-issues, parent, dependencies, a commit's pulls, issue links, related
merge requests, `closed_by` and `closes_issues`, each of which reaches an object in another repo. A
**compare** that names a head in another owner is refused, as is a GitLab `from_project_id` that is
not the recorded one. A pull request or merge request is opened only from a **bare branch or the
recorded owner** (`head_repo` the recorded repo; GraphQL `headRefName` likewise), `target_project_id`
must be the recorded project, a compare side with more than one colon is refused, GitLab's
`repository/commits` routes are exact and its **pipelines** are limited (bridges and variables are
refused). **GitLab text that carries a quick-action line is refused on REST and on GraphQL, every
action and the `/label`, `/unlabel` and `/relabel` family included** (A2 round 6): the scan reads the
text as written and with every `\r` removed, the way GitLab does, and takes the command word as
Unicode alphanumerics, lowercased, refusing a non-ASCII word. **A label named by id (`label_ids`,
`labelIds`, the `add_` and `remove_` forms) is refused in a REST or GraphQL body**: the proxy cannot
read a label's name from its id, so it cannot tell a merge label from another; labels are set by
name through the API's `labels` field, where each is checked (D3e). `epic_id`, `epic_iid`,
`parent_id` and `parent_iid` are refused in a GitLab body.

**What is refused in every request, both forges (A, A2):** a header whose name holds an underscore,
a second `Content-Type` of any spelling (exactly one is sent, the type the body was read by), a
credential as a JSON or form **body key**, a read sent upstream with a body, and the line breaks
U+2028, U+2029, VT, FF and NEL read as ends of line. `release`, `deploy` and `release-*` branches
are protected names. **LFS** is the **download batch only**, answered for a recorded repo and
anonymously for a public one, its storage addresses and credentials held by the listener, and the
chat given addresses on the host it reached; **one file** to GitLab's `uploads` is the only
multipart. LFS from a GitLab object-storage host is refused with the sentence D3f gives.

**A named non-repo read list**, and only that: `GET /user`, `GET /rate_limit`, `GET /meta`,
`GET /user/orgs` and `/user/memberships/orgs` (for readiness), `GET /orgs/:org` for an org the
person has a recorded repo in, `GET /api/v4/user`, `GET /api/v4/version`, and the equivalent
discovery endpoints. Each is a read.

**Refused, whatever the token could do:**

- `/repositories/:id` (the numeric alias) **unless that id is recorded**; likewise
  `/projects/:numeric` on GitLab unless recorded: an alias is a second spelling of a repo, and a
  recorded id is the check that it is the same repo;
- **`/search`** in every form (code, issues, repositories, users): it reads across every repo the
  person can see (**this is REST's `/search`; GraphQL's `search` for exactly one of the chat's
  repos is answered, D3e**);
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
- **`node` and `nodes`** work **for ids the proxy ties to the chat's repos** (round 9 refused them
  whole; the gh and glab documents select them, so they are answered, and an id that cannot be tied
  is refused); **`search`** is answered **for exactly one of the chat's repos** (`gh pr status` and
  a filtered `gh pr list` send it): the `repo:` qualifier must equal the recorded path, there is
  exactly one, printable ASCII only, no quotes, no duplicate argument, no `OR`, `AND` or `NOT` in
  any case and no parenthesis, and no org, user, owner, team or enterprise qualifier;
  `__type` and `__schema` introspection is answered. **Every mutation not on the list is refused**,
  including every merge and auto-merge mutation (`mergePullRequest`,
  `enablePullRequestAutoMerge`, ... : D8), and the mutations that take only label ids
  (`issueSetLabels`, `mergeRequestSetLabels`, `addLabelsToLabelable`, `removeLabelsFromLabelable`;
  D3d);
- **every `owner`, `name`, `fullPath` and `nameWithOwner` argument is checked** against the
  recorded repos, as an ID is, wherever it appears (`repository(owner:, name:)`,
  `repositoryOwner`, GitLab's `project(fullPath:)`, `namespace`): a pair that is not a recorded
  repo is refused;
- **below a root, the selection is default-deny by field.** A **field allowlist per type** names
  the fields a chat may select (`PullRequest.title`, `.body`, `.state`, `.headRefOid`,
  `.statusCheckRollup`, `.comments` and so on); **a field not on its type's list is refused**, and
  so is a field whose type the list does not know. **Every object a response returns is then
  checked on the response**: its owning repository (a node's `repository`, or the parent it was
  reached through) **must be a recorded repo**, or the whole response is withheld and the call
  audited as refused. The list is first-party and short; the response check is the backstop for a
  field the list gets wrong. **Refused by name, and listed so a review sees them:**
  `viewer.pullRequests`, `viewer.issues`, `viewer.issueComments`, `viewer.gists`,
  `viewer.repositories`, `viewer.organizations`, `user(login:)` and everything under it,
  `organization.repositories`, `repository.forks`, `repository.owner.repositories`,
  **`CrossReferencedEvent.source`** (a timeline item that points at an issue or pull request in
  **any** repo the person can read), and GitLab's `group.projects`, `user.projects` and
  `currentUser`'s memberships. A selection the proxy cannot resolve is refused;
- **a related repository is shown as its identity only.** Round 9 refused `repository.parent` and a
  pull request's `headRepository` and `baseRepository`; **`gh`'s own queries select them, so they
  are answered as `RepoName`: the name, the owner login and the default branch name, and no
  `isPrivate` or `viewerPermission`**, and nothing else of the other repo. References that reach
  another repo (`closingIssuesReferences`, `closedByPullRequestsReferences`, both
  `associatedPullRequests` edges, `commits.nodes`) return **reference types** (`IssueRef`,
  `PullRequestRef`) that hold an id, a number, a url and the repo as `RepoName`, and **no title,
  body, state, comments, author, labels or reactions**: "Fixes org/private#N" reveals nothing of
  N. The four edges `gh issue view` selects titles on (`parent`, `subIssues`, `blockedBy`,
  `blocking`) use a `RelatedIssue` type that keeps title and state, because only a relationship
  write creates those edges and the proxy allows none; a human-made cross-repo link still shows
  its title. `ProjectCard.note` and `Project.body` are dropped, and **GitLab's `titleHtml`,
  `descriptionHtml` and `bodyHtml` (and a Release's `descriptionHtml`) are dropped**; GitHub's
  rendered HTML is kept, since its cross-repo links carry no title, only an existence and type
  signal;
- **the field allowlist per type is built** (`forge/proxy/schema.rs`) and **walked through fragments
  and connections**; the gh and glab documents are fixtures (`tests/fixtures/one-login-graphql/`,
  one test per document). They were **transcribed**, not captured off a wire, and the real-tools CI
  job uploads the documents it sees so a refused one becomes a fixture;
- **a document is decided per touched repo**: `graphql::decide` returns the repos a document
  touches, the proxy asks the credential source once per repo and **refuses unless every repo is
  served by the same account** (G), and a document that touches no repo is a non-repo request;
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
  forge or a **known merge bot** reads as a command is **refused** (on GitLab, every quick action
  is: D3d), with the one sentence (the
  proxy reads these as request body data it already parses, not as a harness's output). **A
  label write** whose name is a known merge label is refused the same way, and **a label named by
  id is refused** (D3d). **The list is the known ones only**: a repository's own bot or a
  `labeled` / `issue_comment` workflow with a command nobody told the proxy is a named residual
  (D3h.7). **A review or release whose whole text is `LGTM`, `approved`, `merge` or `queue` is
  refused as a merge command** (`MERGE_LABELS`, J): a false refusal accepted for now; matching a
  merge word only in command position is a follow-up (#7301).

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
- **CI files** are held, not refused (D8.3). **Built (F and F2, #7299): the push is held when any
  path it changes is a CI-file path (D8.3's rule, on the path alone, never by parsing a pipeline
  file).** The diff the person is shown is read in **a quarantine the app owns, never from the chat's
  clone** (a fresh repository, the base fetched from the forge with the person's credential, the
  chat's pack taken in by `git index-pack --stdin --fix-thin --strict` so git hashes every object);
  a source scan fails on any other reader of a trusted object store. The push is buffered (Open item
  5). **`diffscan.rs` is not reused**; the quarantine is new code.
- **A tag pushed with git** (`refs/tags/…` in a push) is **refused by the allowlist**: its commit is
  not on the forge to read, so no ask can show what it names. A tag is asked for through the API (D8.2).
  A push whose commands mix classes is refused as **its most severe class** (protected ref, then a
  ref the forge did not advertise, then a release or tag), and `git-receive-pack` raises **no ask of
  its own** in the app's route, since an ask would hold the command lines and not the pack (J).
- **LFS, download only.** The `locks` API and a batch `upload` operation are not on the allowlist.
  **The batch call is a `POST`, and that one `POST` is allowed on the credentialed route and on
  the anonymous one (D3i)**, only with `operation: download`. A
  **download** batch (`/info/lfs/objects/batch` with `operation: download`) is served: for a
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
  any duplicate of either that disagrees. **A lone `Transfer-Encoding: chunked` request body is
  accepted on the forge route and the anonymous route's upload-pack `POST` only** (D3a: a push
  needs it, and so can a clone's request); any other value is refused.
- **Credentials in a URL are refused on every route and every forge host:** the query parameters
  `private_token`, `access_token`, `job_token` and `token`, and **URL userinfo**
  (`https://user:secret@host/`). A chat does not have a token, so one in a URL is one it found or
  made up, and the forge would honour it as the person's or another's. The request is refused with
  the one sentence and audited; **the same rule applies on the anonymous route** (D3i).
- **One request per connection**: the proxy answers with `Connection: close` and never reuses a
  connection between requests, so a second request cannot ride the first's verdict.
- **One approach for every chat, locked or not, at every floor** (the one exception, a person's
  own No-sandbox chat, is D3b's). A chat **never uses the person's own `gh` or git login**:
  `GH_CONFIG_DIR` is the chat's own folder (no login in it, only the `http_unix_socket` setting,
  D3c), **the credential helper list holds only purlis's one helper, which answers the sentinel
  and holds no token (D3c; round 7's "empty" is replaced, and #7241's emptied list is kept
  except for that one entry)**, `GIT_CONFIG_*` is purlis's, at the watch floor and the full
  sandbox alike, and **the egress proxy denies the forge hosts directly** (`github.com`,
  `api.github.com`, `uploads.github.com`, the GHES host, `gitlab.com`, the self-hosted host;
  **"denies" means the `CONNECT` is answered by the forge route's own termination (D3a), not
  tunnelled to the forge**; **the download-only hosts `codeload.github.com`,
  `release-assets.githubusercontent.com` and `objects.githubusercontent.com` are the one
  carve-out, D3i**), **so the only route to a forge is the one this section describes**, which is
  why D3i exists: a denied host with no anonymous way would break every dependency fetch. **This
  amends ADR 0067's amendment of 2026-11-19**: its *"under the shipped `watch` floor the person's
  `gh` and git over https keep working in a chat, on purpose"* (the keep) is withdrawn, and
  `forgelock::claimed` no longer decides whether a chat is on the lock: every chat is. The class-6
  credential files are denied at the full sandbox as before.
- **An administrator's never-allow pin wins over the route** (the person's ruling, 2026-11-21).
  Where the project's administrator policy pins a forge host (`github.com`, a GHES host,
  `gitlab.com` or a self-hosted GitLab host) as never allowed for chats (`never-hosts`, ADR 0067),
  **a chat is refused that forge entirely, the forge route's listener included.** The hand-off to
  the route (D3a) happens **only after the administrator's never-allow pins are checked**: a
  `CONNECT` or request for a pinned forge host is answered by a refusal and never reaches the
  route's termination, its credential or any layer below it, so no repo list, held act or anonymous
  read (D3i) is ever asked about it. The refusal is **one sentence that names the pin and who set
  it** (as the egress refusal of any pinned host does, ADR 0067) and says that the person's own
  sign-in does not change it. **The person's sign-in does not override an administrator's pin**:
  being signed in, having the repo recorded or trusting the clone grants a chat nothing on a host
  policy has pinned, and the person's own terminal, which is not a chat, is untouched. A host the
  policy does not pin is served by the route as this section says. This order is a fact of the
  proxy's decision, not of the route: the pin is read at every connection, so a pin added while a
  chat runs refuses that chat's next one.
- **What the egress denial is, as built** (B #7292, `sandbox/forgedeny.rs`). It holds a forge **three
  ways, and each is checked on every connection**: **by name** (every name under a forge host:
  `*.github.com`, `*.githubusercontent.com`, `*.gitlab.com`, `*.<a recorded host>`, less the three
  download-only hosts, which are exempt and nothing else; `ssh.github.com` on any port,
  `altssh.gitlab.com`, `gist.*`, `raw.*` and `uploads.*` are denied), **by the name inside a tunnel**
  (every `CONNECT` and SOCKS5 tunnel's first TLS record is read and sent on whole, never decrypted:
  a name other than the exempt ones is refused if its SNI is a forge host or under one, case and
  final dots folded; a first record of content type `0x14` to `0x18` that is not a whole, readable
  ClientHello is refused, as is an SSLv2-compatible hello, since a split or odd hello hides its
  name; an exempt download host is carried **only where the SNI equals exactly the name asked**,
  which closes `raw` through `objects`; **ECH keeps its outer name**, so an ECH hello naming a forge
  is refused, and an ECH or no-SNI hello is refused on a tunnel to an address literal while a forge
  is denied and carried to a named host, so a browser's GREASE ECH keeps working), and **by
  address** (an allowed name whose resolved address is one the forge is at is refused, per resolved
  address, with no time-of-check gap; an address literal the forge is at is refused at decision,
  with NAT64 and IPv4-mapped literals read as the IPv4 address they carry and every numeric IPv4
  spelling `getaddrinfo` reads matched as a literal). The forge's addresses are **a vendored
  snapshot of GitHub's published web, api and git ranges** (`forgedeny/github-ranges.txt`, read at
  build time and never fetched at runtime; **a weekly workflow refreshes it with
  `tools/github-ranges-refresh.mjs` and opens or comments on a `bot-maintained` issue when it
  differs**) plus **a resolution of every recorded forge host and of its sibling names** (`altssh`,
  `ssh`, `registry`, `uploads`, `gist`, `raw`, `www`), accumulated per address and held 20 minutes,
  with a failed refresh keeping the stale set, a failed lookup not asked again for 10 seconds, a
  sibling that does not exist asked again after 10 minutes, and lookups single-flight. **Port 22 to
  any forge name is refused, the exempt names included.** The refusal of a name that resolves only
  into forge addresses names the forge. **Per-host git settings** point git at the chat's egress
  port for each recorded host and each known forge host (`http.https://<host>/.proxy` and the
  wildcard). **A runner's or planned-elsewhere chat is not wired** (`Plan` crosses the session
  protocol and only the local chat has a forge route; denying a runner chat a forge it has no
  route to would only cut its own `gh`), and needs a wire field with the slice that gives a runner a
  route (D9). What this does not hold is D3h.11 to D3h.14.

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
   close it. The window recommends them as advice (D6), never as a condition. **What the hold
   names, exactly (D8.3, F2 #7299):** on GitHub, anything under `.github/` and any `action.yml` or
   `action.yaml`; on GitLab, any `.yml` or `.yaml` file and the project's `ci_config_path`. So **a
   script an existing workflow runs** (`make`, `npm test`, a job's shell script) and **a local
   action's code outside `.github/`** are not held by path: the forge credential bounds them, not
   a hold. **Other CI systems on a GitHub repository are not held at all**: a `Jenkinsfile`,
   `.circleci/`, `.travis.yml`, `azure-pipelines.yml`, `buildkite` files and the like run on
   infrastructure purlis does not know of, and a push that changes them is an ordinary push of a
   chat's branch. (On GitLab the `.yml` rule happens to cover several of those names; it is not
   written for them.)
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

10. **The per-chat CA, and a proxy that reads the forge's TLS.** For the recorded forge hosts the
    proxy terminates TLS under a CA the app minted for the chat (D3a), so **the proxy holds the
    plaintext of every forge call of that chat**: the sentinel on the way in, the person's token
    on the way out, and every pack and body. It already did on the plain-HTTP side; the new fact
    is the key. **The consequence:** a process that obtains the CA's **private key** can present
    a certificate for a forge host to any client that trusts that CA, which is **only a client
    this chat's environment pointed at it** (git's per-host `http.<url>.sslCAInfo`, `GLAB_CA_CERT`),
    never the system, the keychain or the person's browser.
    So: **the key stays in the app's memory and is never written** (the certificate is public);
    **a new CA at every chat start and resume, dropped when the chat ends**, with the leaf
    certificates short-lived; **the CA is mandatorily name-constrained: its certificate carries a
    critical `nameConstraints` extension limited to the recorded forge hosts** (D3g), so a leaked
    key signs for no other host. **Per-client enforcement is a build gate** (Open item 1): `git`
    and `glab` must each be shown to refuse a certificate for a host outside the constraint.
    **A client that does not name the CA fails with a certificate error**, which is the intended
    way for a tool outside the list to find that it has no route. **A chat that copies the CA
    file into its own trust** has gained nothing the proxy does not already see. **The CA is never
    added to the system or keychain trust**, because that would make the key a machine-wide
    interceptor *(spike E also ran the OS-store form on both systems, as a comparison; it is not
    used)*.

**Residuals the reviews accepted.** Each was named by the slice's final review, is fail-closed on
the way it fails or bounded by the forge credential, and is listed so a later reader does not
find it. **D3h.11 to D3h.14** are the egress denial's (B); **D3h.15 to D3h.17** the listener's and
the allowlist's (A2); **D3h.18 to D3h.20** are G, F2 and J's.

11. **Servers that ignore SNI.** The tunnel pin reads the server name in the first ClientHello. A
    GHES or self-hosted GitLab that answers whatever name it is sent is held **only by its address
    lists** (the vendored GitHub ranges, and what each forge host and its siblings resolve to). A
    forge reached at an address that is in neither (a range GitHub adds that nothing resolves into
    yet, or a GHES at an address its name no longer resolves to, or never did from this machine)
    is held by name and SNI only; and **a forge that publishes no ranges** (gitlab.com, most
    GHES) while its lookups fail and nothing is live is **not held by address at all**.
12. **PROXY-protocol ports.** A self-hosted front end that reads a PROXY-protocol header before the
    TLS hello would see the tunnel's first bytes as that header: they are no TLS record, so they
    are carried (to a name that is not exempt), the port inside the preamble is not read, and a
    forge reached that way is held by its address lists only. The same holds for **a server-first
    protocol on a wired tunnel** (tunnel ports are 443 only): its first bytes are no record, the
    wait for a hello ends at the head's 30-second deadline, and it is refused as "sent no readable
    TLS record in time", not as a forge route. **A server that accepts a different server name in
    the second ClientHello** (after a HelloRetryRequest) is pinned to the first name and nothing
    later.
13. **Shared addresses.** A forge behind a shared CDN address (gitlab.com) is held at that address,
    so another site there is refused too, for the hold's 20 minutes; GitHub Pages, `ghcr.io` and the
    `185.199.108.0/22` block share GitHub's ranges and are refused by name with a sentence naming
    the forge, **documented and not carved out by SNI**, because a carve-out would carry any name
    inside a tunnel to a forge address. **An arbitrary non-forge name said inside a tunnel to a
    forge address that no list holds is carried by design.**
14. **What this proxy does not see.** A chat with the network family off, or whose traffic does not
    go through the proxy (a program with no proxy setting, a raw socket where the sandbox allows
    one), is not served by this denial at all: **the sandbox's own network rule holds it**, and a
    chat-set `NO_PROXY` that libcurl applies ahead of the per-host proxy meets that rule, not a
    forge. **A plain-HTTP request through the proxy** has no TLS to pin and is held by name and
    address only; **6to4 and Teredo literals** are not unmapped. **The ranges snapshot ages** (a
    weekly workflow refreshes it and SNI pinning holds meanwhile). **The resolver has no timeout of
    its own** (`system()` is a bare `to_socket_addrs`, so a blackholed forge name stalls a chat's
    new connections for the operating system's resolver timeout: #7301). **The wiring folder of a
    chat is removed when the app next opens the project**, not when the chat ends.
15. **`commits/{sha}` reads fork-network objects by sha.** GitHub shares objects between a
    repository and its forks, and the proxy cannot tell which repo a sha is in, so a read of a
    commit by its id can return a commit of another repo in the network. It is a read of a commit
    an id was already known for. REST **query keys on a listed route are one global list**, not
    one per route, and **forwarded path segments keep the chat's case** (A2).
16. **GHES through the alias, and LFS from object storage.** GHES behind `github.localhost` is
    mapped and not proven by a real `gh`; **LFS from a GitLab object-storage host is refused**;
    `search` accepts one `repo:` qualifier and refuses quotes, so a richer search is refused.
    **The gh and glab GraphQL documents are transcribed, not captured**, and the CI job uploads what
    the real tools send (D3e).
17. **Bodies are held and a few reads are bounded.** A push over 32 MiB and a JSON body over 1 MiB
    are refused; an open URL longer than 4096 bytes in an answer is dropped from the scrub's reach
    with its userinfo skipped, so the hold stays bounded.
18. **G (sign-in).** A GitHub device code from a cancelled CI-permission flow stays approvable until
    it expires (D1.1, #7301); copy that says "nothing changed" after `workflow::begin` cleared the
    scopes is wrong and is a follow-up; **the proxy compares token bytes across the repos a request
    touches** and refuses on a GitLab renewal mid-request, where it should compare accounts
    (#7301); `ForgeRepoId` is type-enforced against bare numbers only; the **D5 scan is a lint,
    with `HostScope` as the wall**.
19. **F and F2 (held acts).** A release on an existing tag sends no `target_commitish` (GitHub ignores
    it when the tag exists); GitLab's `execute_filemode` drop is not reproduced; **a push through the
    API is one commit on the base** (the commits between are not sent, and its id is not the tip's);
    a GitLab project record that does not carry `ci_config_path` refuses the ask; **a GitHub
    release or tag sent as a form whose body is JSON slips past the merge-command walk** (walk the
    JSON and the pairs whatever the content type, or refuse form asks off GitLab paths: #7301);
    a GitLab `unapprove` is refused and is not an ask, so the question of pinning its head does not
    arise.
20. **J (the app's route).** Reattached chats and chats that outlive the app have no route (D3a). The
    route is made **only if** the listener's socket is mode `0600` and every accepted connection's
    peer uid is checked, **each chat's sandbox is denied `pf-<uid>/` except its own socket**, and
    the recorded repos and account bindings sit where nothing a chat writes reaches them, because
    `perform_held` rereads them at the act: these are conditions the other slices meet, and
    one that fails widens what a press allows. **The runner pairing (`RunnerLink`) is a stub the
    pools follow-up owns**, so a runner's chat keeps its token connections until it lands.

### D3i. The anonymous read route: dependency fetches with the forge hosts denied

With the forge hosts denied at egress (D3g), a `git clone` of a **public repo the person has not
recorded**, and the tools that fetch from a forge **through git** or through the proxy, would fail.
**An anonymous, read-only pass-through, inside the same port and the same secret, carries what is
designed here, and no credential is ever injected on it.** **What is designed:** `git` (and
anything that runs it with the environment's git configuration: `go get` with `GOFLAGS=-mod=mod`
and `GOPROXY=direct`, `pip install git+https://…`, `npm install github:…`, which call `git`), `gh`
and `glab` (D3c), and the download-only hosts below. **What is not claimed:** that every tool a
chat might run fetches this way. A tool with **its own HTTPS client** (`curl`, `wget`, a package
manager's tarball fetch, an installer pulling a release binary) does not read git's configuration
and goes to `CONNECT`, which is ruled in the last bullet (**`npm` and `pip` tarballs from
`codeload.github.com` are tunnelled through `HTTPS_PROXY`, never terminated, and carry no
credential**: D3c *(spike E)*); **`cargo`'s built-in git client
(libgit2, `net.git-fetch-with-cli = false`, the default) got 401 through this route on both
systems in every variant** *(spike E)*, so **`net.git-fetch-with-cli = true` is set for every chat**
(D3c) and libgit2 is a named gap.

- **Unrecorded public repos only.** A recorded repo (D4) keeps the credentialed route; the
  anonymous route is **chosen by the proxy, from the repo in the request** (D3c: no path prefix
  and no rewrite), serves a repo that is not recorded, and
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
- **Storage hosts are dialed only for an exact URL the proxy was handed, never one the chat
  chose.** The storage hosts (GitLab's GCS or S3 buckets, `objects.githubusercontent.com`) are not
  addressable as a request's own target at all. The proxy dials one **only for the exact URL it
  received in a forge's redirect `Location` or in an LFS batch `href`** on this request chain, once,
  with the credential dropped, and holds the URL in memory for that fetch. A chat that names a
  storage URL itself gets a refusal. For GitLab's buckets that keeps a signed URL of another repo
  from being replayed through the route; **for GitHub it is a narrower guard**, because `CONNECT`
  to `objects.githubusercontent.com` is open (last bullet) and a chat can fetch such a URL
  directly. There the rule keeps the route's audit and its closed list, not secrecy.
- **`GET` and `HEAD` only, with two `POST`s that read.** The upload-pack `POST`
  (`git-upload-pack`) **and the LFS batch `POST`** (`/info/lfs/objects/batch`, whose body must say
  `operation: download`; **an `upload` operation, `locks` and every other `POST` are refused**).
  Everything else is refused. The same canonicalisation, the same header stripping and the same
  audit as D3d and D3g, **and the same refusal of credential query parameters and URL userinfo
  (D3g)**.
- **Tools with their own HTTPS client: the `CONNECT` ruling.** The egress proxy still denies
  `CONNECT` to the forge hosts that take a credential or serve the API, but **allows `CONNECT` to
  download-only hosts that never take a credential**: **`codeload.github.com`,
  `release-assets.githubusercontent.com` and `objects.githubusercontent.com`**. *(Spike E, `curl
  -I`, 2026-11-20: a release download at `github.com/<o>/<r>/releases/download/…` answers `302` to
  **`release-assets.githubusercontent.com`**, not `objects.githubusercontent.com`; an archive at
  `github.com/<o>/<r>/archive/…tar.gz` answers `302` to **`codeload.github.com`**. Both are on the
  list; `objects.githubusercontent.com` stays, as older assets and other flows still name it, and
  is not in either redirect today.)* The list also covers GitLab's archive and package download paths **where they
  are on separate hosts** (a GitLab archive on the instance's own host cannot be separated and
  stays on the route above). That serves `curl` and `npm`, `pip`, `go` tarball fetches and an
  installer's release binary **when the URL names such a host**. **`CONNECT` to `github.com`,
  `api.github.com`, `uploads.github.com` and the GitLab hosts stays denied as a tunnel**, so **`curl
  https://github.com/<o>/<r>/releases/download/…` fails**: `curl` is not told the per-chat CA, so
  where the forge host is reached through the terminating route it ends in a **certificate
  error**, and where it is not recorded or the tool sends no sentinel it takes **the anonymous
  route, which reads only what the world can**. It gets no credential in either case, and the
  denied tunnel raises a Notice whose one sentence says *"Use `gh release download`, which goes
  through purlis, or the `release-assets.githubusercontent.com` address the release redirects
  to."* (`gh release download` goes through the proxy and the redirect rule of D3g.) **The
  download-only hosts are tunnelled, never terminated**: a plain allowed `CONNECT`, the real
  certificates, no per-chat CA and no credential. A download-only host takes no credential and
  an arbitrary path, so a `CONNECT` to it carries no person's access; **it is also not a way to
  read a private repo**, for the host serves only what a signed or public URL names.
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
("Trust this clone", for a clone that already exists). **ADR 0077 amended 2026-10-29's E3** is *"the
github.com remotes of the clones the chat's workspace had at chat start"*; that amendment is
superseded here (D1.2), and the supersession **keeps E3's meaning** (the chat's repos are the
clones the person has) **while D4's list, with its confirmation, host and numeric id, is new
code**, not E3's. A clone made from the CLI is outside it (#7196) until the person trusts it. The
confirmation records the clone, the host, **the owner/name and the numeric repo id** (a rename or
a transfer does not carry the id, and an alias that resolves to the same id is the same repo; a
lookalike with another id is not), and the **registration's client id** that signed in, so a
lookalike application is not mistaken for purlis's. Submodules and forks are recorded the same
way, on the person's press.

**Built (G, #7288).** `forge::confirmed` records, on the press (a clone from the window, *Trust
this clone*), the host **and port**, `owner/name`, **the numeric repo id read from the forge as the
person** (never taken from the clone or the request) and the account that serves it, in
`forge-repos.json` (Machine, device-bound) beside the trust list (`push-remotes.json`, same
`clone_key`). A repo is served only if both name it, **by number**: the id the proxy recorded for
the chat's repo must be the confirmed one, so a repo renamed away and replaced under its old name
by another is not served, and a request with no id is refused. The trust list's address must still
be the entry's, and the **port is part of the address** (a token confirmed for `host:443` is never
sent to `host:8443`). The application the repo was recorded under must still be the host's pin, or
the account and the repo are refused and a `PinChanged` Notice is drawn. **Several accounts** are
chosen per project (Settings › You › Accounts, *Account for each project*); with two on a host
the answer is kept as "choose", the repo is recorded with no account and nothing serves it; **a
signed-out chosen account is never replaced by another**. `ChatRoute` is the proxy's credential
source: the chat's own clones, `confirmed::serves`, `account_for`, then `SignedIn::credential_of`,
and every other case is a refusal. **Headless**, `headless_repos` reads `PURLIS_FORGE_REPOS` or the
checkout's `origin` in memory, with the numeric id from `GITHUB_REPOSITORY_ID` or `CI_PROJECT_ID`,
fails closed, and `reconcile` intersects it with what a desktop confirmed. Non-repo requests are
refused unless every repo of the chat on that host is served by the same account.

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
   **Built (J, F2): an ask is raised only for an act that replays exactly.** The route raises one
   **only for a recorded repo of its run**, read from the REST path's own segments, and for
   a merge **only when the request carries a non-empty `sha`**, which is the head the ask pins;
   otherwise the chat gets a plain refusal that says to send it. **A GraphQL mutation raises no
   ask** (core stops at the first refused field, so a bundled document could be labelled by one
   field and replayed with others), and neither does a `git-receive-pack` that is not a CI-file
   push (D3f). The ask holds the exact request body and content type, shown to the person verbatim
   and performed once; **asks are keyed by the digest of the whole act**, so different acts are
   different asks, and the chat's 403 names the ask (`Ask 3: tag v1.4.0 in acme/app.`). **Limits:** a
   run holds at most 16 asks, a chat raises at most 20 per ten minutes and the app 60 across
   chats, refused before any forge read past that. **An approval is refused, never held** (a GitHub
   review with `APPROVE`, GitLab's `approve` and `unapprove`): F2's acts are the merge, tag, release,
   fork and CI-file push, and J sends every held act through F2's sink only, so there is no ask an
   approval could raise.
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
pushes a tag. **Built (F2):** a release names its tag and **never sends a `target_commitish`**; a
tag is a ref on GitHub and `repository/tags` on GitLab; a rev given for a tag or release must be a
40- or 64-hex id or a valid ref name (both adapters refuse anything else before any forge call, so
a `..` path never reaches the forge), and the notes and title have a raw byte bound before they are
queued. A tag name with a direction or zero-width character is refused.

### D8.3 Pushes that touch CI files

**Which pushes are CI-file pushes: by path, never by parsing (F and F2, round 6 of #7299).** Round
9 of this record named CI files by what a pipeline includes (`include:` of `.gitlab-ci.yml`) and
by a tree diff. Five rounds of review each found input on which the YAML parser purlis used and
GitLab's own loader read a file differently (merge keys, tags, byte-order marks, line breaks,
`---`, symbol keys, key interpolation, multi-document splits), and each fix narrowed the gap
without closing it, because the decision depended on two readers agreeing on every file a chat can
write. **The decision is on the path alone**, and no pipeline file is parsed. A push is held if it
**adds, changes, deletes or renames** any of these (a rename is the removal and the addition it is,
so both paths count):

- **GitLab:** any file whose name ends in `.yml` or `.yaml`, in any case, anywhere in the tree
  (a GitLab `local:` include must be a YAML file, so this covers every local include without
  reading one), `.gitlab-ci.yml`, and **the project's configured `ci_config_path`**, read from the
  forge's project API as the person at each ask and never cached. A project record that cannot be
  read, or that does not carry the field, **refuses the ask** with a sentence that says why; purlis
  does not guess the default;
- **GitHub:** **anything under `.github/`** (workflows, local actions, reusable workflows and what
  they reference) and **any file named `action.yml` or `action.yaml`, in any case, anywhere**,
  which a `uses: ./path` can name;
- **both:** the dependency bots' configuration and `CODEOWNERS`.

**The trade-off, accepted:** on a GitLab repository a push that edits only YAML nothing includes (a
data file, a Helm values file, an OpenAPI document) **asks for the person's press where it did not
need to**. An extra ask is visible and cheap; a pipeline read differently by two parsers is not.
**What the rule cannot name** is a file a pipeline or workflow runs that is not YAML and not under
`.github/` (a script a job calls, a local action's `index.js` outside `.github/`) and **any other
CI system's files on a GitHub repository**: they are not held (D3h.6), and the forge credential
bounds them.

**How the check works, and what it costs.** The app receives the push's pack and reads it **in a
quarantine it owns** (D3f): a fresh repository, the base fetched from the forge with the person's
credential, the pack taken in by `git index-pack --stdin --fix-thin --strict`, so git hashes
every object and a loose object the chat overwrote in its own clone changes nothing the quarantine
shows. The base is the forge's tip of that branch, **or the default branch's tip for a brand-new
branch**, so every CI file the new branch adds against the default branch is in the diff. The same
check covers every other way a ref moves (a REST create-ref or update-ref, GraphQL `createRef` and
`updateRef`, a commit already in the repository's fork network), **and if the diff cannot be
computed** (an object missing, a pack too large to buffer, a listing over 16 MiB, a
`ci_config_path` the forge will not say) **the push is refused, not allowed and not guessed**. Every
git read is capped while it is read (a listing 16 MiB, a file 1 MiB), and the list of paths and the
diff body escape control and bidirectional characters before they are drawn.

If a path matches, **the pack is held in the app, the ref is not updated, and the Ask shows the
diff of the CI files** (the changed lines, not only their names), the branch, **the sanitised, bounded
commit message and a count of the non-CI files**. On approval **the app pushes**: **on GitHub it is
one commit on the base the person was shown, parented on it, with the ref moved not forced** (the
lease is read first, so a branch rewound to an ancestor of the base is refused), **using the second
token (D1.1) for every call it makes**, used only here and for no route of a chat. **On GitLab it
uses the everyday sign-in** (there is no second token), with the branch read right before and
after. A push through the API is one commit on the base: the commits between are not sent and its
id is not the tip's (D3h.19). **Where the second token is absent or unknown the act is refused with
the sentence that says to push from the person's own terminal.** Because a pushed pipeline file
runs code as the person (GitLab) or with the repository's secrets (both), **a CI-file push is never
approved by default and never remembered**: each one asks. **The hold does not cover what an
existing workflow runs on a branch push** (D3h.6).

### D8.4 Where no one is present

A held act with nobody to ask (an unattended runner, ADR 0124 §6) is **refused with the sentence
"Held: nobody is here to approve it."** and queued as an ask on the person's desktop if one is
connected (a **new** queue of asks on the desktop: ADR 0077 amended 2026-10-29's E4 is a pool's
cluster mint and has no such queue). Nothing is performed unattended.

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
as the only destination**, and whether the host allows it. **The fork's source must be a
recorded repo (D4)**: a request to fork any other repository is refused, not held, so the person's
account is not used to copy a repo the chat merely named. A fork request that names another
organization or namespace is refused, not held. On approval the app creates the fork as the person
and records it (D4). **Built (F2):** the fork names no organization or namespace, and the adapter
binds the **source repo's id** on both forges, so a repo replaced under the same name is not forked.

## D9. Token connections: credentials where no person signs in

**No person signs in on a runner.** With the desktop present, a runner's forge calls are
**forwarded to the desktop app's proxy** (D3a): the runner holds nothing, the desktop's sign-in
serves them, ADR 0078's V9 intent (runner needs no forge credential) holds. **Headless** use (CI,
`purlis serve`, the operator's long-running host) has no desktop and **uses a token connection
only**. The proxy, its allowlist and its refusals are **identical**; only the credential's source
differs.

**What is built (C #7273, #7294, #7297; H #7289; J #7295) and what is not.** The **credential
sources are built** (`docs/credential-sources.md` is the reference for the names, the token kinds
and the renewal): per host, in order, `PURLIS_FORGE_TOKEN__<HEX>`, `PURLIS_FORGE_TOKEN_FILE__<HEX>`
(a path, for a mounted secret), then the forge CLI's own variables, **each offered only to the host
it names** (`GITHUB_TOKEN` to github.com only when `GITHUB_SERVER_URL` is unset or names it, `CI_JOB_TOKEN`
to the `CI_SERVER_HOST`, and so on), and **a token's kind is read from its exact prefix and shape**:
`ghs_`, `github_pat_`, `glpat-` and the job token's forms are taken, **`ghu_` and `gho_` are taken
only from a `PURLIS_FORGE_TOKEN__<HEX>` variable**, a classic `ghp_` and a `ghr_` refresh token are
refused, and an unrecognised token needs `PURLIS_FORGE_ACCEPT__<HEX>` and a host that is neither
github.com nor gitlab.com needs `PURLIS_FORGE_KIND__<HEX>`. **The mode picks the source, and it
picks one:** the window reads the signed-in account only (a `GITHUB_TOKEN` exported where the
window started is nobody's choice of identity); a headless run reads a token connection for that
host, and the machine's sign-in answers only when none is given; **a token that is given and
unusable is the answer, and nothing falls back**. A long-lived token's expiry is given as
`PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>`, and a warning comes 14 days before it. **GitHub sign-ins are
renewed by the app** where the host expires them (GitLab's two hours: before they run out, by one
process at a time, the rotated pair written to the keyring before the new token is used; only the
forge's `invalid_grant` or `bad_refresh_token` ends a sign-in; **no machine lock on Windows**). **Not
built: desktop forwarding.** `RunnerLink`, the runner pairing, is a stub the pools follow-up owns, so
**a runner's chat keeps its token connections** until it lands, and **the runner is not wired for the
egress denial either** (D3g). **Retired (H): the mint port** (the desktop tells no runner of it, a
runner takes no word of one and follows no key roll) and the `mint = "github-app"` tool line (D3h
and Migration).

**Desktop forwarding: the transport and who the runner is. Specified here, not built.** The runner's own per-chat proxy
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
`CI_JOB_TOKEN`, `GITLAB_TOKEN`, `PURLIS_FORGE_TOKEN*`, **the checkout's authentication**, **and
`/run/secrets`** (the chat's sandbox does not mount it).

**The checkout's token is on disk, not in the environment.** `actions/checkout` writes
`http.<host>.extraheader` (the job token, base64) into the clone's `.git/config` and, for
submodules, into their configs and the includes it adds. **Before a chat starts, the runner
removes every such entry, and the CI's credential-helper entries, from the clone's config and its
includes** (D3c's carve-out: this is the CI's credential in a runner-owned checkout, not the
person's configuration), and **the start fails closed** if it cannot read the config to clean it
or an entry is still there on a re-read. **The proxy reads the tokens from its own process
environment** (D9.2), and **a sandboxed chat must not be able to read it**: the chat's sandbox
**denies reading another process's `/proc/<pid>/environ`** (and `ps -E`, `/proc/<pid>/cmdline` of
the proxy) on Linux, by running the chat in its own PID namespace or under a rule that denies the
proxy's `/proc` entries. **No such rule is named in ADR 0067 today, so this is a requirement of
the headless profile, not a citation**: a headless chat does not start where the profile cannot
show it (Open item 14). A chat run with no sandbox (D3b) can read it, which D3h.5 says. A token is **accepted only if its type is
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
(ADR 0128 D3b)"*; item 4's exception names **a chat's own port and its own unix socket**; item 5 gains *"except
the forge route's requests (over the chat's own unix socket, or inside a `CONNECT` the proxy
terminates for a recorded forge host), which are read"*; item 7 gains *"except the
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
| **Agent identity** (`forge/agentidentity.rs`, `apptoken.rs`, `AgentIdentityFlow.tsx`, Settings › Agent identity, the Pool setup step, the `secret exec` `gh` wrapper, the App's key in the vault) | built | After the first start post-upgrade, a person who is signed in has every chat on the proxy at once; one who is not sees *Sign in to github.com*. The org's App stays installed on GitHub, untouched, and a Notice tells an owner they may uninstall it. **purlis removes its own record and, on the person's yes, the vault key**: never silently. All of it is deleted. **H built the retirement, behind the same switch** (see *What H built*, below): the setup flows refuse, the removal paths still work, and **nothing is rewritten or deleted for the person**; **the dead mint code stays until the switch goes** |
| **Committed `mint = "github-app"` lines** | in project files; **teammates on older builds** still read them | On the new build the line is **ignored and a Notice says so once** (*"This project asks for a GitHub App token that purlis no longer mints; chats use your sign-in"*); the line is not rewritten on the person's behalf, but a window action deletes it from the project file. A **teammate on an older build** keeps working as before (the line still mints there); the two builds do not disagree about the repo, only about the credential. The line is removed from `purlis init` templates now and from the docs; support of the key is dropped one release after this record's feature ships |
| **Forge lock slices #7205–#7216** | #7205, #7206 closed; #7207–#7216 open | See the next table |
| **Design PR #7171** (the merge and release lock is the forge credential) | open | **Closed as superseded** by this record: its premise (the forge locks merges for a bot) is gone (D6). Its facts on rulesets and bypass actors move to the research note as the advisory backstop (D6), not a design |
| **#7241** (a chat on a locked repo holds no path to the person's forge login) and **`forgelock::claimed` without the mint** | merged | **Kept and generalised**: its routes (a `GH_CONFIG_DIR` holding no login, the helper list emptied (**round 8: but for purlis's one sentinel helper**, D3g), `GIT_CONFIG_*`, no SSH agent, the connector denies) become the every-chat baseline (D3g); `claimed`'s *claim* test is removed, so every chat is on it, **with the 0067 amendment above**. The "Not closed" rows of that amendment (a chat's own `-c credential.helper`, keychain) are what the egress denial of the forge hosts (D3g) closes, and what D3h names where it does not |
| **#7230** (GitLab works under the credentials class through a brokered `glab` route) | **merged to `main` (`7d184670f`) while this record was in review**; its route hands a chat a vault GitLab token | **Retired by H behind `forge-wiring`**: with the switch on, `glab` is left out of a chat's tools by name and by the program it resolves to, a vault item recorded as a GitLab token (`gitlab-token-…`) is refused to a chat, and the guarded `glab` shims point to the sign-in. **Its fallback is the person's, not a chat's:** where no OAuth application can be registered for the host (D1.3) the token becomes **a token connection** (D9) the person pastes in the window, kept in the keyring; the vault item is then offered for deletion on the person's press, never deleted for them |
| **#2053** (Runners: agent forge identity for `gh` and `glab`; SD-7a/b) | open | **Replaced** by desktop forwarding and token connections (D9). Closed as superseded; the pool and operator path is D9.2's Secret |
| **#3882** (mint a chat's GitHub App token on a GHES) | open | **Not marked met on paper.** It is met by D1.3 (a GHES registration) and D3 **only once proven**: a live sign-in against a GHES, through the proxy, with the readiness check, passes slice E's spike on that host. It stays open until then, with the proof named |
| **#7133** (the merge and release lock is the forge credential) | open | Closed as moot (D6), after its useful slices are re-parented (next table) |
| **#1351** (purlis answers git's HTTPS credential requests for sandboxed chats) | open | Closed as superseded: purlis answers no credential request. **Round 8's helper answers only the sentinel, a value worth nothing outside the proxy (D3b, D3c)**, so the issue's own design, a helper that returns the person's token, is not built |
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

**What H built (#7289), with the switch on.**

- **Tool lines.** `mint = "github-app"`, a vault value in any forge token variable (`GH_TOKEN`,
  `GITHUB_TOKEN`, `GITLAB_TOKEN`, `OAUTH_TOKEN`, `CI_JOB_TOKEN`, `PURLIS_FORGE_TOKEN*`, and the
  rest of the scrub list, D3a) and any tool called `gh` or `glab` (by name and by the program it
  resolves to) still parse, but **the tool is left out and runs nothing**; the refusal says to sign
  in under Settings › Forges. No mint is called and no chat's environment holds a forge token. A
  committed `mint` line is **not rewritten**; where the App's key is still in a vault, the sentence
  says to keep the line while it is (deleting the line unmarks the key) and to remove the key on the
  person's press, and `purlis doctor` does not warn for ever about a line kept on purpose.
- **Vault items.** `get_value` (so `secret exec`, `get` and `cp`) **refuses a chat** an item purlis
  recorded as a forge token (Agent identity's reserved prefix, GitLab access's `gitlab-token-` items)
  by name, and any item whose value carries a forge token prefix, in the `--env`, `--file` and
  `--dotenv` forms. The person at their own terminal is not refused; the broker's and a runner chat's
  reads are marked as a chat's.
- **Runners and setup.** The mint port is dead (above). Agent identity's `create_app`, `keep_key`,
  `add_lines`, `write_and_allow` and GitLab access's `set_up` refuse; their removal paths work. Pool
  setup's identity step gives no tool line to add.
- **The once-only Notice.** `forge_replaced` carries the text and a cause key that moves only with
  the project's lines; the window draws it with **Open sign-in** and a Dismiss kept per project until
  the lines change. `purlis doctor` gains a `forge sign-in replaces` row.
- **Residuals.** A token an MCP server reads from its own variable (`GITHUB_PERSONAL_ACCESS_TOKEN`,
  which the repo does not list) is not refused, and a program that is not a forge's, under the
  person's Allow, is handed what the Allow names. **Every shipped binary, the runner image included,
  must be built with `forge-wiring` once the train turns it on**: a binary built without it still
  takes the old routes. **The brokered git push route that used the person's `gh` or `glab` login is
  retired by slice K, not H.**

**New stores.** Each names its tier in `docs/plane-format.md` before it ships (ADR 0069): the
per-host access and refresh tokens (**Keyring**, `purlis/@forge/…`, as ADR 0077 §7); the
**per-host registration store** with its pinned client id and owner (**Machine, device-bound**,
nothing secret, rebuildable by signing in again); a token connection's secret **is the operator's
own** (a file or environment that purlis reads and never stores; a desktop-pasted PAT is a
**Keyring** item); the held-act asks (**Clone state**, rebuildable: a held pack is memory only, so
a restart drops it and the chat re-asks); the per-chat secret (memory only, not a store). The
`agent-identity-gitlab-` store of round 1 is not made. **Built, each with its tier in
`docs/plane-format.md`:** `forge-repos.json` (the confirmed repos with their ids, **Machine,
device-bound**), the second token (a **Keyring** item), `forge-registrations.json` (corrected to
**device-bound**), a chat's wiring folder (`.purlis/app/forge-wire/chat-<pid>-<random>/`, the app's
state, swept with the ssh routes), a run's route folder (`pf-<uid>/<pid>-<hex>/`, swept by pid
liveness), and the held-act quarantine (app-owned, rebuildable).

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
| 9 | A helper answers anyone who runs it | The one helper holds no token, answers only the sentinel, and no-ops `store` and `erase` (D3c) |
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
| **The chat does `gh`/`glab` and native git fetch, pull and push, pull and merge requests, comments and CI reads, nothing typed** | D3c: **`gh` through `GH_HOST=github.localhost` and a per-chat `http_unix_socket`, with the wrapper passing `-R github.localhost/<owner>/<repo>`; git through `HTTPS_PROXY`, a per-chat CA and a helper that answers the sentinel; `glab` through `HTTPS_PROXY`, `GLAB_CA_CERT` and `GITLAB_TOKEN`; no rewrite, `git remote -v` unchanged. Condition met on Linux and macOS for these mechanisms, against a fake forge and a stand-in proxy** *(spike E, #7270: `gh` 2.103.3 and 2.105.0, `glab` 1.119.0 and 1.131.0, `git` 2.56.1; 52 and 50 of 66 cases work, the rest listed as not proven)*. **Gates that remain:** `gh repo clone` and a full `pr create`, `pr checks`, `pr checkout`, `run view` and a paginated list, **against the real proxy and a real forge**; GHES; the name constraint on the CA (D3h.10) | what the wrapper gives up: `gh`'s inference of the repo from the remotes and a bare `gh pr view` or `pr status` (`-R` needs the pull request argument); **cargo's libgit2 (401 in every variant), a named gap with its Notice**; a `gh`, `glab` or `git` version spike E did not run gets a Notice; if `gh` fails the gates above, `gh pr create` on GitHub is the named gap and the pull request is opened by the app on the person's press, as ADR 0078's V9 amendment says |
| **A chat can merge, tag, release, change CI, fork** | D8: asks that show exactly what will happen, approved in Asks, performed by the app with the SHA bound; a CI-file push on GitHub uses a **second, `workflow`-scoped token** the chat's routes never see | none held dead. **Not closed:** what an existing workflow runs on the chat's branch push, merge-bot labels and comments the proxy was not told, deploy-trigger branch names it was not told, the keychain `gh` token at the watch floor (use stopped, exfiltration not), and a chat's own words asking the person to open a sign-in page (D3h.6 to D3h.9, D3j); **other CI systems on a GitHub repository, a script or local action code outside the CI-file paths, and the egress denial's and listener's accepted residuals (D3h.11 to D3h.20)**. **A CI-file push on GitHub is not offered if GitHub folds the second token's scope into the first** (D1.1) |
| **Dependencies still fetch** | D3i: an anonymous read-only route for unrecorded public repos, release and asset downloads and public LFS, with no credential ever injected; recorded repos keep the credentialed route. **npm and pip tarballs from `codeload.github.com` are tunnelled, never terminated**, through `HTTPS_PROXY` with no CA variable set *(spike E)*; **cargo git dependencies** go through git with `net.git-fetch-with-cli=true` *(spike E)*; the download-only hosts are `codeload.github.com`, `release-assets.githubusercontent.com` and `objects.githubusercontent.com` | a private unrecorded repo is refused, as it would be to anyone. **Exceptions (D3i):** `curl https://github.com/<o>/<r>/releases/download/…` fails with a certificate error or takes the anonymous route, and a Notice says to use `gh release download`, and **cargo's libgit2** (401) is a named gap with its Notice. A live LFS clone is not proven |

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

1. **What spike E did not prove, and still gates the build** (D3c). **Since round 9 the train has run
   the real `gh`, `glab` and `git` through the real listener to a fake forge on Linux and macOS**
   (`.github/workflows/one-login-proxy-listener.yml`: every tool installed, the proxy refusing
   nothing the tools send, and each refusal case (another repo, a merge, `gh auth token`) failing at
   the proxy and never reaching the forge); **a real forge is still not run**, and a real GHES is
   not. Against the **real proxy** and then a **real forge**, `gh repo clone` (by `o/r` and by URL, and what it writes to `git
   remote -v`), `pr create`, `pr checks`, `pr checkout`, `run view`, a paginated list and a
   GraphQL query through the wrapper's `-R`; GHES; **a live LFS download clone** (D3f, D3i); and
   **libgit2** (401 in every variant: a named gap, its cause open); **per-client enforcement of the
   per-chat CA's critical name constraint** (D3h.10: `git` and `glab` each refuse a certificate
   for a host outside it, on both systems); **a test that #7241's helper reset precedes purlis's
   helper** (D3c). **Proven by spike E on both
   systems, and no longer open:** remote matching (ruled: `-R`), reach and the sentinel swap for
   `gh`, `glab` and git, and cargo with `net.git-fetch-with-cli`.
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
5. **The CI-file hold buffers a pack** (D8.3): the limit is set (a push is held up to 32 MiB, the
   quarantine's listings to 16 MiB); **its latency on a large push is not measured.**
6. **The GraphQL allowlist** (D3e) is a list someone keeps; **its first list is built** (A2) from
   transcribed gh and glab documents, and the CI job's uploaded documents are how it grows.
7. **Token-connection limits** (D9.4): each row is confirmed by slice C's probe.
8. **Two forge hosts in one chat** works for git and `glab` by the request's own host; for `gh`
   the wrapper picks the host from the working directory (D3c), and a second virtual host beside
   `github.localhost` was **not run** by spike E.
9. **ADR 0124 has no "decision 20"** in this checkout; D8.5 names the items (§7 items 3, 4, 5, 9
   and §7.2) and the amendment file cites those.
10. **LFS upload and locks** are refused (D3f); **LFS download is served** (D3f, D3i), and whether
    a live LFS clone completes is Open item 1's probe. **Several accounts on one host**, a
    person who leaves an organization and a revoked authorization each show as a Notice with a fix.
11. **Desktop forwarding's channel** (D9): the runner's existing authenticated connection is named
    from ADR 0078 by reading, not by running; slice A confirms the key, the pairing and that no
    new listener is needed before the forwarding ships.
12. **`glab`'s credential header, `gh`'s and git's** (D3b): **confirmed by spike E** for each on
    both systems, against a fake forge; **a re-run on each `gh`, `glab` or `git` bump** is the
    pin's rule (D3c), and the proxy's own tests confirm the swap against the real proxy.
13. **The anonymous route's closed host list** (D3i) and **the merge-bot and label lists** (D3e)
    are lists someone keeps; their first versions are slice A's.
14. **The headless chat's `/proc` denial** (D9): the profile that denies a chat the proxy's
    `/proc/<pid>/environ` is named by no ADR today, so it is a requirement, and slice E's spike
    confirms on Linux that a chat cannot read it.
15. **The follow-ups of the final reviews (#7301), none blocking and each fail-closed:** record "CI flow
    pending until expiry" and treat the account as unknown meanwhile (D1.1); correct the copy that says
    "nothing changed" after `workflow::begin` cleared the scopes; run a read-back when a CI-permission
    code is dead, or say in the changelog that none runs; carry the account id on the proxy's
    credential and compare accounts, not token bytes, across a request's repos, then narrow
    `Secret::expose` to `pub(crate)`; cap the resolver (D3h.14); say in `credential-sources.md` that a
    forge with no published ranges is not held by address while its lookups fail; walk a JSON-bodied
    form for a merge command and match merge words only in command position (D3e, D3h.19); give the
    held-act digest's doc comment its due.
16. **Settings tabs and removal presses (H).** Agent identity's and GitLab access's old settings tabs
    still draw their UI (their setup refuses); a window press to remove the vault item or the Agent
    identity record is not built.
17. **The e2e stand-in for the chat wiring** (`seams.mjs` `makeChat`) is not replaced by
    `forgewire::compose` (B): it needs `gh`, `glab`, `openssl` and a loopback bind, which the slice's
    sandbox could not run. The shape is a `wire` op on the Rust bridge that returns `compose`'s
    environment, pairs and `bin`.

## Amendments

An amendment to this decision is a file of its own in [`0128/`](0128/), named
`<yyyy-mm-dd>-<slug>.md`, never a section added to this record (#1982).
