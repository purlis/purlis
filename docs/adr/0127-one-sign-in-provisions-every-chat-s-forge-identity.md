# One sign-in provisions every chat's forge identity

**Proposed 2026-11-20**, for the person's requirement: *a person who has signed in to a forge
account with purlis runs a chat, and the chat does `gh`, `glab` and native `git fetch`, `pull`
and `push` with no command the person runs by hand.* It follows the steward's fixed decisions of
2026-11-20 (numbered D1 to D6 below). The facts, with their sources, are in
[`docs/research/one-login-forge-identity.md`](../research/one-login-forge-identity.md). Nothing
here is built.

## Where purlis is

- A person signs in once per forge host (ADR 0077). The token is theirs, and never reaches a chat
  (FI3).
- A chat's `gh` is handed a minted token only where the person has done a second, separate
  setup: Settings › Agent identity, or Pool setup, which registers a private GitHub App and
  installs it (ADR 0077, amended 2026-10-29). A chat's `glab` is handed a vault token the person
  pasted (#7230).
- A chat's **native git has no credential at all** since #7241: its helper list is emptied, its
  ssh agent is empty, and `gh` finds no login. purlis's own git helper (ADR 0070, amended
  2026-10-12, #752) answers only the git the window runs, never a chat's.
- The brokered `purlis worktree push` still authenticates with the person's own `gh` or `glab`
  login on the machine (`gitbroker.rs:239-257` → `pieceupdate.rs:1863` → `sync.rs:147-157` →
  `forge::helper_for`).

So a locked chat can call `gh` and cannot push; and what lets it push is a manual setup, a pasted
token, or the person's own login. That is the gap.

## The decision

**One sign-in provisions, per forge host and owner, a narrow identity that the app mints for each
chat. A purlis-run git credential helper, `gh` and `glab` all answer with that one token, for the
hosts and repositories of the chat's own clones and no others.**

### D1. The person's token stays out (FI3 kept)

Nothing about the sign-in token changes, and no chat is handed it or anything it can mint. The
sign-in flow *uses* it, in the app, to create the chat identity (GitLab) and to find the owners
(GitHub). It does not copy it.

### D2. Native git gets the narrow token from a purlis helper

- The chat's git is started with a purlis credential helper named in the per-chat git
  configuration the sandbox already passes in the environment, after #7241's empty
  `credential.helper` (so no other helper sees the token), with `credential.useHttpPath`
  on.
- The helper is the app's binary started again by git. It asks the app, over the hook socket,
  for `get` only. The app answers only **HTTPS**, only for the **host and path** of a repository
  the chat's own clones have (the list ADR 0077 amended 2026-10-29 E3 builds), and only with the
  chat's own token. `store` and `erase` answer nothing. Every answer is audited with the chat's
  id.
- **ssh remotes are rewritten to HTTPS** for those same hosts by git's `url.<base>.insteadOf` in
  that per-chat configuration (both the `ssh://` form and the scp-style form), and the ssh agent
  stays shut (#7241). The clone's own config on disk is not touched.
- The **brokered `purlis worktree push`** and `fetch` take the same token from the same source
  for a chat caller, instead of `forge::helper_for`'s CLI helper. A chat's branch push can no
  longer reach the person's `gh` or `glab` login. The window's own git keeps the person's account
  (unchanged).
- `gh` and `glab` get the same token as `GH_TOKEN` or `GH_ENTERPRISE_TOKEN` with `GH_HOST`, and
  `GITLAB_TOKEN` with `GITLAB_HOST` (unchanged for `gh`; the same shape for `glab`).

This **replaces #1351's** "answers from the operator's own helper or keychain": the helper answers
with the chat's own narrow identity and never reads the person's helper, keychain or sign-in.

### D3. GitHub: option (b), the per-owner manifest flow, inside the sign-in

**Recommended and proposed: (b).** A purlis-published App with a token service (a) is refused:
GitHub tells software on a user's device to "never ship your private key", and a service holding
a key that signs for every installation of purlis's App is a single breach of every customer's
repositories, plus a server that git depends on. ADR 0077's V16c refused it for the same reasons
and nothing found since changes them. The facts are in the research note, sections 1.3 and 4.

What changes from today's flow:

1. **It runs inside the sign-in**, not in Settings › Agent identity. After the GitHub sign-in
   succeeds, the window continues straight into the manifest step for the owners it will need.
   Settings › Agent identity stays as the place to repair, add an owner or remove the identity.
2. **One App per person per host, public, owned by the person's own account**, instead of one
   private App per owner. A private App "can only be installed on the account that owns the app"
   (research 1.2), so today's private, per-owner App cannot serve a person who is not an owner
   of an organization, and one per organization means one manifest round each. A public App owned
   by the person can be **installed on any owner by that owner**, or **requested** by a member
   (GitHub sends the owner the request). The App name is `<login>-purlis-agent`, as today. The key
   goes to the vault as today, so only this person's machine can mint, and the App is useless to
   anyone else who installs it. A person who prefers an organization-owned App chooses it in
   Settings, which keeps today's route and its private manifest.
3. **The same permissions**: `contents:write`, `issues:write`, `metadata:read`,
   `pull_requests:write`, and `workflows:write` only if the person asks. No webhook, no events.
4. **Installation is the second click**: the person installs the App on each owner their clones
   are on, and picks all or some repositories. A chat's token is limited to its own clones inside
   that (ADR 0077 amended E3).

### D4. GitLab: the sign-in creates a project or group token through the API

- After the GitLab sign-in, for each project the chat's clones are on, purlis calls `POST
  projects/:id/access_tokens` with the person's sign-in token, in the app: role **Developer**
  (30), the scopes `api` and `write_repository`, and an expiry of 30 days. It is stored in
  the vault as #7230 stores a `glab` token, under a reserved key so only the mint reads it
  (as ADR 0077's amendment E2 does for the App key). A **group** token (Owner role) is used
  where the group's projects are many and the person is the group's Owner.
- It is rotated by the rotate call before it expires, and revoked with the identity.
- Because GitLab has no short-lived token, the GitLab identity is **per project, shared by that
  project's chats, for days**, not per chat and an hour. This is GitLab's limit (research 2.1)
  and is accepted for SD-7b.
- The token is a bot identity at the Developer role. It pushes to a protected branch only where
  the project's rule allows Developers, and a chat's work goes to a branch and a merge request.

### D5. Hosts

github.com, gitlab.com and self-hosted GitLab first. GitHub Enterprise Server next (#3882): the
manifest flow and installation tokens exist there (research 1.5), the App is registered on that
host, and the mint sends the JWT to that host's API alone.

### D6. No rights: say so, ask the owner, never fall back

- GitHub, member not an owner: the install step becomes GitHub's **request**; the chat starts with
  a Notice naming the owner and the repository, "waiting on the owner of `<org>` to install it",
  with a link to the request page and Copy for the message. A repository admin installs it
  themselves where GitHub allows (research 1.2).
- GitLab, not a Maintainer or Owner: no token is created. The chat is told which role is needed
  and from whom. GitLab.com **Free** has no bot token: see the open gap 2.
- **No chat is ever handed the person's login.** `gh` and `glab` see their empty config folders
  (#7241), the helper answers nothing for a repository with no identity, and the brokered push
  refuses with the same sentence, so a missing identity can be told from a missing
  network.

## The person-visible flow

**GitHub, github.com, first time on a machine**

1. The person clicks *Sign in with GitHub*. The window shows a code and opens
   `github.com/login/device`. They approve it. *(Today's step.)*
2. The window says *Set up what chats use to push* and lists the owners of the project's clones
   (their own login, then their organizations). They press *Continue*. If they have signed in
   before and an App exists, this and the next two steps are skipped, and only step 5 runs for a
   new owner.
3. The browser opens GitHub's *Register new GitHub App* page, filled in. They press **Create
   GitHub App**. *(Click one, the least GitHub allows: there is no API that creates an App.)*
4. GitHub returns to the window. The window stores the key in the vault, opens the App's
   installation page, and waits.
5. On that page they choose the owner (themselves, or an organization), choose all or some
   repositories, and press **Install** (a member who is not an owner sees **Request**, and the
   window says it is waiting on the owner).
6. The window shows *Done*. From here a chat in those repositories does `git fetch`, `pull`,
   `push`, `gh pr create` and `gh issue` with nothing typed and nothing asked.

**What a chat sees.** `GH_TOKEN` holds a one-hour installation token limited to its repositories.
`git push origin HEAD` works; git asks the helper, and the helper answers with that token. `git
remote -v` still shows the clone's own remote, and an ssh remote works too, since git rewrites it
for that chat. `gh auth status` says the token is an app's. A repository the App is not installed
on says so in one sentence, naming the owner and the request that is waiting.

**GitLab, first time**

1. The person clicks *Sign in with GitLab*, approves in the browser (PKCE). *(Today.)*
2. The window lists the projects of the clones and asks nothing more than *Create access for chats*.
   They press it. No browser page opens: the app creates the tokens through the API.
3. A project they may not manage shows *Needs a Maintainer* (and a group, *Needs an Owner*), and
   the chat is told.

**A runner chat** (ADR 0078) takes its token from the desktop as today; a pool that holds the key
itself is #2053 and ADR 0077 amended E4, and is not changed here.

## What it supersedes or amends

- **ADR 0077, V16c, wording.** *"A GitHub App the organisation registers itself"* becomes *"a
  GitHub App the person or their organisation registers, from the sign-in."* *"Charter's shared
  GitHub App (FW-1) is for sign-in only and its private key never ships"* **stands**: purlis's
  own registration still mints nothing and holds no key. The 2026-10-29 amendment's E1 (the
  declaration is a persona's tool line) is kept, and its E2 (the key is a vault value only the
  mint reads) is kept for both forges. This is an amendment file under `docs/adr/0077/`, never a
  section of the record.
- **ADR 0077, FI3.** Unchanged in its words. Sharpened by this record: FI3 is about the
  *sign-in token*, the sign-in flow may use it in the app to set the chat's identity up, and the
  chat's identity is the only credential any chat route (`gh`, `glab`, native git, brokered push)
  has.
- **ADR 0077, SD-7b "GitLab, for now".** "A token the organisation makes" becomes "a token the
  sign-in makes"; the vault storage of #7230 is its storage.
- **ADR 0070, amendment 2026-10-12-02 (#752).** "Never inside a chat's process tree" stays for
  **the person's helper**. A second helper, for the chat's own identity, **is** in a chat's
  tree, and answers only the chat's token. This record amends that file's wording to say so.
- **ADR 0067's credentials class.** Unchanged: the class still denies the person's login. This
  record fills the lock with a way to do the work, which #7241 left with none.
- **#1351** closes with this record's D2. **#7215's** design 3 ("GitLab.com Free takes a
  Developer user's token") is not adopted until the open gap 2 is ruled. **#2053's** SD-7a/b is
  built by this record's flows. **#3882** is D5's GHES half.

## Consequences

- A person signs in and sets up once, and every locked chat in those repositories can push. That
  is the requirement, with the exceptions below.
- **GitHub needs two browser confirmations** (Create, Install) after the sign-in's own approval.
  There is no way to fewer: the REST API has no create-App endpoint, and installing is the
  owner's act.
- A purlis-signed-in person's chats mint with a key the person holds, in the vault, as today.
  There is no purlis server and no purlis key.
- The helper adds a code path that answers a secret to a process in a chat's tree. The answer is
  the token the chat's `GH_TOKEN` already holds (accepted in the 2026-10-29 amendment), limited to
  its repositories, to HTTPS and to the chat's own hosts, and audited.
- A public App per person is visible on GitHub under the person's account and can be installed by
  anyone, and nothing follows from that, since only the key mints.
- A locked repository (#7205, #7210) is not made easier to get into: the lock refuses a mint for
  a repository that is not locked, and the helper answers nothing in that case.
- New store: **none.** The App's key and the GitLab token are vault values under reserved keys
  (`agent-identity-github-`, and a new `agent-identity-gitlab-`), in the vault tier
  `docs/plane-format.md` already names (Keyring or a vault provider's own store). That file
  changes first, with a **Tier:** line for the GitLab prefix, and
  `every_store_the_plane_format_names_has_a_tier` covers it.

## Open gaps

1. **Does GitLab's create-token call accept the sign-in's OAuth token?** The API page says "You
   must use a personal access token with this endpoint". If it does not, D4 cannot be built as
   worded, and the fallback is a prefilled GitLab page (a click and a paste, which the steward's
   D4 rules out), or asking GitLab for the call. A probe against gitlab.com and a self-managed
   server comes first.
2. **GitLab.com Free has no project or group token.** Only the person's own fine-grained
   personal token (GitLab 19.2 and later, Free) can be limited to a project, and it acts as the
   person. D6 forbids the person's login as a fallback. **Needs a ruling:** either Free is
   unsupported ("Needs Premium" in a Notice) or a fine-grained token is accepted as the narrow
   identity, with an ADR amendment.
3. **A branch protected by a pull-request rule** refuses an installation token unless the App is
   on the bypass list, and a Developer GitLab token unless the rule allows it. The chat's
   message says which.
4. **A clone made from the CLI** is left out of the chat's repositories (#7196). The helper
   inherits that rule, and #7196's three asks (say it at `purlis clone`, mark it in the Repos
   panel, tell the chat) become this feature's.
5. **Pools.** A runner without its desktop needs a minted token from somewhere (ADR 0077 E4,
   not yet built). The helper on a runner uses whichever source E4 gives.
6. **GHES** (D5): the helper, the manifest flow and the mint on a non-github.com host, with the
   JWT sent to that host alone (#3882). The paths on a server older than 3.22 are unchecked.
7. **Several accounts on a host**, a person who leaves an organization, an App the person deletes
   or a key that is rotated: each must show as a state in Settings › Agent identity and as a
   Notice with a fix, not as a failed push.
8. **Revocation at chat end** stays (`DELETE /installation/token`). A GitLab token is not
   per-chat and is revoked with the identity or on expiry.
9. **`workflows:write`** is off by default. A push that touches a workflow file fails with
   GitHub's message, and the Notice should offer the permission change, which every
   installation's owner must approve.
10. **Unverified:** a repository admin who is not an owner installing the App (research G3), and
    the exact GitLab scope names (G6).
