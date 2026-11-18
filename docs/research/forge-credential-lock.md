# The merge and release lock is the forge credential (#7133, Q331)

Status: design, **round 2** (two independent reviews said "sound with changes" and "build with
changes"; this revision designs in or accepts as a residual every point they raised). For the
person to grill. No implementation code, no cargo, no live forge was touched.
Date: 2026-11-18. Rulings it rests on: Q331 (the lock is the credential; the shell-text guard
stays as the early warning), Q330 (an API ref update of the default or protected branch is held
as a merge). Related: #7092, #7124, #7131, #7118.

**The goal, restated.** A chat can never **merge into a protected branch**, and never create,
move or delete a protected tag or publish a release, by any route, without the person's press.
"Protected branch" means the default branch plus the patterns the person lists, **including every
branch that publishes** (nightly, dev, release/**, gh-pages) because those are merges in effect
(section 6.4). The lock is the forge's refusal of the chat's credential; it holds only while
**the chat holds no credential the forge lets through** (section 6.1).

**How to read the facts.** Every forge fact is marked **[doc: URL]**, the page it was read on
(docs.github.com and docs.gitlab.com only, 2026-11-18), or **[verify]**: no page read says it, so
slice 0 (section 11) settles it on a throwaway repo before anything is built. Round 1 marked some
facts "[inferred]"; they are now **[verify]**, since an inference is not a basis. Nothing marked
**[verify]** is a basis for a decision. Code facts cite `file:line`.

## 0. Round 2: what changed, by point

| Review point | Where |
|---|---|
| B1 the person's login; locked requires the credential class; bypass mode `pull_request` | 6.1, 7, slice 1, 13 (decision 1) |
| B2 releases | 6.2, test rows 12.2 |
| B3 auto-merge and merge queue | 6.3, 7 (gaps) |
| B4 the CI route | 6.4, 7 |
| B5 verify run first, script and answers | 11 (slice 0), 11.1 |
| Goal as "merge into a protected branch" | the top |
| Every repo of the workspace; live tokens; `revoke_now` | 8 |
| Gap states | 7 |
| Acceptance and lock cache in a store no chat writes | 7.2 |
| Exact permission allowlist; nested `permissions` through `Field` | slice 2, 10.1 |
| Extend `ruleset_protects`; one answer for the merge ask and the lock | 7.1 |
| App's login cannot set the lock; org rulesets not buildable | 9 |
| GitLab has no chat credential | slice 10, 4 |
| Slice 7 and 8: tag and release seam, assets, signed tags, bound to a commit | 10.3 |
| Unverified claims (`merge-async`, `actor_type: User`, `exempt`) | 2, 11.1 |
| Cost to the person | 9 |
| Migration | 10.2 |
| A check that needs no admin (`current_user_can_bypass`) | 7.3 |
| Test rows | 12 |
| Slice order | 10 |
| Open decisions with recommendations | 13 |

## 1. What a chat's credential can do today

From the repo:

- The App `purlis-purlis-agent` is made from a manifest asking `contents:write`, `issues:write`,
  `metadata:read`, `pull_requests:write`, and `workflows:write` **only if the person ticked it**
  (`crates/purlis-core/src/forge/agentidentity.rs:140-150`, `Wanted::permissions`; the box is
  `Wanted::workflows`, `agentidentity.rs:113`). **Workflows is opt-in, not off**: where the person
  ticked it, a chat can push workflow files (section 6.4).
- A chat's token is minted per chat and clone: the App's JWT, then
  `POST /app/installations/{id}/access_tokens` with `repositories[]` and **nothing else**
  (`crates/purlis-core/src/forge/apptoken.rs:566-573`, `mint_over`). It carries every permission
  the installation has [doc: <https://docs.github.com/en/rest/apps/apps>, "if `permissions` is
  not specified, the installation access token will have all of the permissions that were granted
  to the app"]. The key sits in vault `agent-identity` (`sandbox/tools/mint.rs`); the chat gets the
  token in `GH_TOKEN`, through the brokered `secret exec` (`secrets/brokered.rs`).
- The merge hold (`mergehold.rs`, `app/src-tauri/src/mergeholds.rs`) merges with **the person's
  own forge login** in the window, never the App's token. The release floor (`floorguard.rs`)
  only reads command text.
- **A chat with no mint falls back to `gh`**, the person's own login
  (`crates/purlis-core/docs/forges.md:58`: "A `purlis` command and a chat always use `gh`").
  That is the largest hole and it is section 6.1.

What GitHub says the installation token can do
[doc: <https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps>,
each row read from the permission's table, "write", installation tokens allowed]:

| Effect | Endpoint | Permission |
|---|---|---|
| Merge a pull request | `PUT /pulls/{n}/merge` (and `/merge-async`: **[verify]**, the page read lists the first; the second is round-1 text and is unconfirmed) | **Contents: write**, not Pull requests |
| Merge a branch | `POST /merges` | Contents: write |
| Write a file to a branch | `PUT /contents/{path}` | Contents: write |
| Create, update, delete a ref | `POST/PATCH/DELETE /git/refs[/{ref}]` | Contents: write |
| Create a tag **object** | `POST /git/tags` | Contents: write. It returns 201 and writes an object, **not a ref**, so it creates no tag by itself; round 1 listed it as a refused effect, which was wrong. The tag exists only when a `refs/tags/…` ref is created |
| Create, edit, delete a release | `POST /releases`, `PATCH/DELETE /releases/{id}`, asset upload | Contents: write |
| Rename a branch | `POST /branches/{b}/rename` | Contents: write |
| Change the default branch | `PATCH /repos/{o}/{r}` | **Administration: write** |

So `contents:write`, which a chat needs to push its own branch, is also the permission that
merges, tags, releases and renames. The App does not hold Administration, so changing the default
branch is refused to its tokens **[verify]**.

## 2. What GitHub can narrow, and what it cannot

1. **A token can be narrowed per request, downward only**
   [doc: <https://docs.github.com/en/rest/apps/apps>, "Create an installation access token for an
   app"]: `repositories` or `repository_ids` (up to 500) and a `permissions` object; "the
   installation access token cannot be granted permissions that the app was not granted"; the call
   needs the App's JWT.
2. **Narrowing cannot split push from merge**: both are `contents:write` (section 1), so any
   token that can push can merge, tag and release **[verify]** (slice 0, row M1). `contents:read`
   stops all of it, including the chat's own pushes.
3. **A ruleset's bypass list names actors, not tokens**
   [doc: <https://docs.github.com/en/rest/repos/rules>]. `actor_type` is `Integration`,
   `OrganizationAdmin`, `RepositoryRole`, `Team`, `DeployKey` or `User`; `actor_id` is required
   for `Integration`, `RepositoryRole`, `Team` and `User`, ignored for `OrganizationAdmin`
   (which "is not applicable for personal repositories") and null for `DeployKey`;
   `bypass_mode` is `always`, `pull_request` ("only applicable to branch rulesets", not for
   `DeployKey`) or `exempt` ("rules will not be run for that actor and a bypass audit entry will
   not be created"). **`actor_type: User` is accepted by the API [doc] but the creating page's UI
   list of eligible actors names roles, teams, apps and Dependabot**
   [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository>],
   so whether a single user can be the bypass actor is **[verify]** (slice 0, row B3). The eligible
   list on that page: repository admins, organization owners and enterprise owners; the maintain or
   write role or custom roles based on write; teams (not secret teams); GitHub Apps; Dependabot.
   An `Integration` is the App, so two tokens of one App with different `permissions` are the same
   actor **[verify]** and the "two tokens" option fails with one App.
4. **The rules that matter**
   [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets>]:
   Restrict creations, Restrict updates, Restrict deletions, Block force pushes, Require a pull
   request, Require status checks (an entry may name an `integration_id` the check must come
   from, [doc: rules page]). Rulesets target branches and tags by fnmatch pattern; the
   `include` list also accepts `~DEFAULT_BRANCH` and `~ALL`
   [doc: <https://docs.github.com/en/rest/repos/rules>]; several rulesets are aggregated, the
   most restrictive rule wins
   [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets>].
   Up to 75 rulesets per repository and 75 organisation-wide (same page).
5. **Plan** [doc: about-rulesets]: rulesets are in public repos on Free and in public and private
   repos on Pro, Team and Enterprise Cloud. Organisation rulesets are for Team and Enterprise
   [doc: creating page].
6. **Enforcement** [doc: creating page]: *Active* enforces on creation, *Disabled* does not.
   The API also has an evaluate state, and "get rules for a branch" omits rules from rulesets in
   "evaluate" or "disabled" [doc: <https://docs.github.com/en/rest/repos/rules>]. A ruleset that
   is not active is therefore a gap (section 7).
7. **Legacy tag protection** is a redirect to rulesets
   [doc: <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/configuring-tag-protection-rules>].
8. **Rename and default branch**: anyone with write can rename a branch "unless it is the default
   branch or covered by a branch protection or ruleset"
   [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/renaming-a-branch>];
   changing the default needs admin
   [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/changing-the-default-branch>].
   Whether an installation token's "role" counts as admin there is **[verify]**.
   **Renaming an unprotected branch into a protected pattern** is a separate row (12.1).
9. **Releases** [doc: <https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository>]:
   people with write "can create, edit, and delete a release". There is no release permission
   apart from Contents, and a release is **not a ref**, so a ruleset does not cover it (6.2).
10. **Auto-merge** [doc: <https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request>]:
    "People with write permissions to a repository can enable auto-merge for a pull request"; the
    repository setting must allow it; the option shows only on a pull request that cannot merge
    yet; auto-merge is disabled if someone **without write** pushes to the head branch or switches
    the base. On a branch requiring a merge queue the repository setting does not apply, and
    "Merge when ready" enables auto-merge even if the repository does not allow it. The page does
    not say **as whom** the merge is made: **[verify]** (slice 0, row A1).
11. **Merge queue** [doc: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue>]:
    once a pull request has passed the required checks, "a user with write access" can add it to
    the queue; repository administrators require a queue through "Require merge queue" in a branch
    protection rule; available in public org-owned repos and in private repos on Enterprise Cloud;
    it cannot be enabled with a wildcard in the protection rule's branch pattern; its temporary
    branches are `gh-readonly-queue/{base}`. Whether the queue's rules are in a ruleset (the page
    says "branch protection rules or rulesets") and whether a non-bypass actor can enqueue under
    Restrict updates is **[verify]** (row Q1).
12. **Immutable releases** [doc: <https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases>]:
    once published, the release's tag "cannot be moved … and cannot be deleted while the release
    exists"; if the release is deleted the tag may be deleted but its name cannot be reused;
    assets "cannot be modified or deleted"; the title, notes, pre-release and latest flags can
    still be edited; a release attestation is generated; the recommended flow is draft, attach
    assets, publish. The page that says how to turn it on
    (`/repositories/releasing-projects-on-github/preventing-changes-to-your-releases`) did not
    load on 2026-11-18, so **who can enable or disable it, and at which level, is [verify]**
    (row R5). It protects what is **published**; it does not stop a chat creating a new release
    with a new tag, or creating and editing **drafts** (not stated either way, **[verify]**, R6).
13. **Installation token `permissions`** [doc: apps page, above]: omitted means all. The key is
    `permissions` with the permission name and `read`/`write`; the installation listing returns
    the installation's own set under `permissions`. This is what slice 2 sends.
14. **`current_user_can_bypass`**: the field the task names was **not found** on the REST
    rules page read, so whether and where it exists, and what it returns for an installation
    token, is **[verify]** (row C1). Section 7.3 designs a no-admin check around it and falls back
    to what the page does say: "get rules for a branch" works with an installation token holding
    only Metadata: read, and returns the active rules at every level, repository and
    organisation [doc: rules page].

## 3. GitLab

- **Protected branches** [doc: <https://docs.gitlab.com/user/project/repository/branches/protected/>]:
  *Allowed to merge* controls merge requests and who creates new protected branches; *Allowed to
  push and merge* controls pushes to existing protected branches. Roles (No one, Maintainers,
  Developers + Maintainers); groups and users are Premium and Ultimate only. The default branch is
  protected by default with Maintainers allowed to push.
- **Roles** [doc: <https://docs.gitlab.com/user/permissions/>]: push to, delete and manage protected
  branches, manage protected tags: Maintainer and Owner. Creating unprotected branches: Developer.
- **Protected tags** [doc: <https://docs.gitlab.com/user/project/protected_tags/>]: creating or
  deleting a protected tag needs the *Allowed to create* list; `*` protects every tag; roles are
  Free, groups and users Premium.
- **Releases** [doc: <https://docs.gitlab.com/user/project/releases/>]: Developer and up can
  create, update and delete; a release on a protected tag needs the right to create that tag.
- **Default branch** [doc: <https://docs.gitlab.com/user/project/repository/branches/default/>]:
  Maintainer or Owner.
- **Project access tokens** [doc: <https://docs.gitlab.com/user/project/settings/project_access_tokens/>]:
  a Maintainer or Owner picks a role and scopes; GitLab.com needs Premium or Ultimate.
- **CI is wider than GitHub's** **[verify]**: `.gitlab-ci.yml` runs on the chat's pushed branch
  with that project's **unprotected variables** and any runner that accepts unprotected refs. Read
  the pages on protected variables and protected runners before slice 9 (not read in round 2: the
  person allowed the hosts, the time went to GitHub). Until then: GitLab's CI route is open.
- **Net:** the role is the lever GitHub lacks. A Developer token cannot merge into, push to or
  delete a protected branch, or create a protected tag, while the project's settings name
  Maintainers or No one. **The app has no GitLab chat credential today** (`not_minted_on`;
  there is no GitLab App to mint from), so this is a new feature, slice 10, not a setting.

## 4. Options compared

| | A. Rulesets: no bypass for the chat's credential | B. Broker proxying the forge API | C. Two tokens |
|---|---|---|---|
| **Security** | The forge refuses on every path (REST, GraphQL, git, `gh`, `curl`) **if** the chat holds no credential on the bypass list (6.1) and the routes in 6.2 to 6.4 are closed | Covers only traffic through it; git over https and ssh, GraphQL's surface, direct connections stay open. A new parser of API calls is what Q331 says is never complete | One App is one actor, so only works with two Apps; then it is A with a second bypass App |
| **Setup** | Per repo or org: two rulesets and a bypass entry; GitLab: protected settings and a Developer token | A proxy and network policy for everything else | A second App |
| **Cost** | Plan: private repos need Pro/Team/Enterprise; see section 9 for who is hurt | Code for every endpoint and transport | A second App key |
| **Harness-agnostic** | Yes | In principle | Yes |
| **Fails how** | Closed, but only for credentials it names | Open | As A |

## 5. Recommendation

**Adopt A on GitHub and the role model on GitLab. Do not build a broker.** Changed from round 1:
"locked" is no longer a property of the rulesets alone. **Locked = the rulesets are right (7) AND
the chat's credential class is enforced with no fallback (6.1) AND the routes 6.2 to 6.4 are
closed or accepted for this repo.** In order:

1. The forge, not text, is the lock. The text guard stays as the early warning.
2. GitHub: a branches ruleset (default branch, listed patterns, **and every publishing branch**)
   with Restrict creations, updates, deletions and Block force pushes; a tags ruleset on all tags
   with Restrict creations, updates, deletions.
3. The bypass list holds **the person only**, as the narrowest actor the forge accepts (decision
   13.1). The chat App is never on it. Bypass mode: **`pull_request` on the branches ruleset,
   `always` on tags** (6.1 explains; tags cannot use `pull_request` [doc: rules page]).
4. The app's own acts (merge, tag, release, default-branch change) use the person's login in the
   window, as the merge hold does.
5. The chat's token keeps `contents:write` and pushes its own branches. Its `permissions` is an
   **exact allowlist** (slice 2). Administration stays off. Workflows is opt-in; where on, 6.4
   applies.
6. **No path from a chat to the person's login** is the first build slice, tested per harness.
7. GitLab: protected settings, a Developer-role project token (a new feature), the app acts as the
   person.
8. The app reads the lock with a state per gap (7), shows it, can set it, and then refuses an
   unlocked mint (after a release of warning only).
9. Slice 0 is run before anything else, on a throwaway repo, by the person's decision (13.3).
10. The optional gate App is built only if slice 0 shows a non-bypass merge **passes** (row M2).

**Why not B**: unchanged: every route a broker could refuse, the forge can refuse itself, and the
git protocol bypasses a broker.

## 6. The routes a ruleset does not close

### 6.1 The person's own login (blocker 1)

Bypass is the person's repository-admin role. A chat that holds the person's credential is a
bypass actor, and merges, tags and releases as the person. Routes, from the reviews:

| Route | Why it exists today |
|---|---|
| `gh` with no minted token | `forges.md:58`: a chat falls back to the person's `gh` |
| The real `gh` binary by absolute path, past purlis's shim | A shim is a PATH entry |
| git over https | The chat's git keeps the person's `credential.helper` (osxkeychain, store, `gh auth git-credential`) |
| gh's macOS keychain item | Not a file, so ADR 0067's file list does not hold it |
| ssh agents named in `~/.ssh/config` (`IdentityAgent`, 1Password, a socket) | The class allows reading `config`; a socket is not a file |
| The purlis browser tools | A profile logged in to the forge is the person's session |
| Harness connectors | A GitHub connector of Claude Code, Codex or OpenCode carries the person's OAuth |

ADR `docs/adr/0067/2026-11-16-the-persons-own-credentials-are-a-denial-class.md` (Q325) holds the
**file** class (`secrets.credential_files = "enforce"`) at the full sandbox, and at the floor only
where the policy enforces it; under the shipped `watch` floor the person's `gh` and git over https
keep working, on purpose. This design therefore **needs an amendment of ADR 0067** (an amendment
file, per CLAUDE.md, not a section): for a repo whose lock is being claimed, the chat's
*environment* (not the person's terminal) carries none of the above. It does not require
`credential_files = "enforce"` for everything the chat does; it requires the **forge credential
class**: the files in the ADR plus the six non-file routes, each closed as follows.

- **No fallback.** A chat on a repo with a mint never falls to `gh`; a chat with no mint gets **no
  forge credential** and the refusal says to mint (changes `forges.md:58` and the code behind it).
- **git:** the chat's git runs with `credential.helper=` emptied then purlis's own helper
  (`forge/gitcredential/`), through `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM` pointing at a file
  purlis writes, and `GIT_TERMINAL_PROMPT=0`. Whether every harness lets purlis set this env is
  **[verify]** per harness.
- **gh:** a chat's `GH_CONFIG_DIR` is an empty folder and `GH_TOKEN` is the mint; the macOS
  keychain item gh uses is denied by the Seatbelt profile (the keychain's mach service)
  **[verify]** that Seatbelt can deny it without breaking the chat's own git. The absolute-path
  `gh` is then the same `gh` with no credential: that is why the keychain denial, not the shim, is
  the control.
- **ssh:** the chat's `GIT_SSH_COMMAND` is `ssh -F /dev/null -o IdentityAgent=none
  -o IdentitiesOnly=yes` and `SSH_AUTH_SOCK` is unset; an allowed agent socket (D-sock-7) is
  refused for a chat on a locked repo.
- **browser tools:** `browser_login` to a forge host is refused for a chat on a locked repo, and
  the browser profile a chat drives never holds the person's forge session **[verify]**.
- **connectors:** per harness, the forge connector is disabled in the config purlis compiles for
  the chat (Claude Code `enabledMcpjsonServers`/connectors, Codex and OpenCode MCP config).
  **[verify]** each by name; a connector purlis does not know is a residual.

**The test is a probe, per harness** (slice 1): purlis starts the harness's chat as it would for
real, with a probe prompt or a scripted shell, and the probe tries each route above (`gh auth
status`, `/usr/bin/…/gh api user`, `git ls-remote` of a private repo with the person's helper,
`security find-internet-password`, `ssh -T git@github.com`, `curl` with no token, a forge
connector's list tool) and the pass is that **none returns the person's identity**. It runs for
Claude Code, Codex and OpenCode, on every harness version purlis has not probed (the result is
recorded with the version), and fails closed: an unprobed harness version is **not locked**.

**Residuals, accepted, with the reason.** A credential the person keeps outside the paths and
variables above (a token in a dotfile, a different keychain, an `~/.ssh` key outside `~/.ssh`)
and a connector purlis cannot name: no sandbox can list what it was not told about. The status
says "locked for the routes purlis knows".

**Bypass mode `pull_request`, evaluated.** With the admin actor in `pull_request` mode
[doc: rules page, creating page], the person must open a pull request and then choose to bypass
at the merge; a **direct push to the branch by the person is refused**. Benefits: the person's own
mistakes and a leaked admin credential cannot push to `main` or rewrite it, the merge leaves a
pull request trail, and the app's merge (a pull request merge) still works **[verify]** (row B2).
It does **not** fix blocker 1: a chat with the person's login can still open a PR and merge it as
the person. It also cannot apply to tags [doc: rules page], so tags stay `always`. Recommendation:
`pull_request` on branches, `always` on tags (13.1).

### 6.2 Releases (blocker 2)

Rulesets guard refs. A release is a separate object that a `contents:write` token can **create on
an existing tag** (no ref is created, so Restrict creations never fires), create or publish
drafts, edit, delete, and replace assets. Round 1's row "`POST /releases` with a new tag is
refused by the tag ruleset" is true only for the new-tag case and the existing-tag case
**succeeds**. Options, with the one chosen:

1. **Immutable releases** [doc: immutable-releases page]. Protects published releases: tag
   locked, assets locked. Does not stop new releases on a *new* tag nor drafts; whether the App
   can switch it off is **[verify]** (R5). Cheap, additive, **recommended as a layer**, set by
   the person's login (an admin setting), and the lock check reads whether it is on.
2. **A `contents:read` chat route**: no push, no merge, no release. Breaks "you can push
   yourself" (2026-11-10) and every harness that pushes with git. Rejected as the default; kept as
   a per-repo option for a chat that only reviews.
3. **The app's own act** (slice 8): create and publish the release as the person, after a press
   bound to a commit (10.3). The chat can still create *drafts*; a draft is invisible to users and
   can be deleted.

**Chosen: 1 + 3, and the rest is a named residual.** A chat holding `contents:write` can still
create a release on a **new** tag name it can create (tags are guarded by the tag ruleset, so it
cannot; this is why the tag ruleset must cover all tags, `**`), can create and edit **drafts**
(R6), and can edit a published immutable release's notes and flags
[doc: immutable-releases page]. Accepted: none of these changes what users download, except the
notes of a published release, which can be reverted by the person and are logged. If slice 0 shows
a draft can be **published** by the chat, the lock check lists "releases publishable by a chat" as
a gap and the repo is "locked except releases" until the app's act is built.

### 6.3 Auto-merge and merge queue (blocker 3)

If the person turns on auto-merge for a pull request, or the repo uses a merge queue, a chat's
**later push** can complete the merge **as the person**: auto-merge merges once the requirements
pass [doc: auto-merge page], and a person with write can enable it; the chat has write. A queue
merges what is added once the checks pass [doc: merge-queue page]. Neither needs the chat to hold
a bypass credential. The lock check names **both** as gaps:

- *Auto-merge is allowed on the repo* (a repository setting, [doc: auto-merge page]): a gap while
  on; the fix is the person's admin setting, `Allow auto-merge` off. A chat enabling it on its own
  pull request is the route; whether it can (row A1) and as whom the merge is attributed (A1) are
  slice 0 items. A person who wants auto-merge for their own pull requests accepts the gap
  (per-repo acceptance, 7.2).
- *A merge queue is required on a protected branch* [doc: merge-queue page]: a gap; "Merge when
  ready" can also enable auto-merge on a repo that does not allow it [doc: auto-merge page], so
  the check cannot stop at the repository setting.

The **mitigation that holds even when the person enables auto-merge** is a required review or
required status from an actor the chat is not (the gate App of 13.1's alternative), because
auto-merge only completes when requirements are met. That makes the gate App useful beyond the
merge-under-Restrict-updates question; it is still built only on evidence (rows M2, A1).

### 6.4 The CI route (blocker 4)

A chat with `contents:write` pushes a branch. On GitHub, that can run code in the repo's
workflows **without** `workflows:write`:

- **Existing workflows run on the chat's push or pull request** and call scripts in the repo
  (a `Makefile`, `scripts/*.sh`, a test file), which the chat can edit. Those runs get
  `GITHUB_TOKEN` and, for `push` events on non-fork branches, the repo's secrets.
- **`repository_dispatch`** is triggerable with contents:write; **`workflow_dispatch`** by a job
  with `actions:write`. Both start workflows whose `GITHUB_TOKEN` may be `contents:write` by the
  repo's Actions default **[verify]** (no page read; round 2 did not read the Actions pages).
- **`Wanted::workflows` is opt-in**: where ticked, the chat can edit the workflow files directly.
- **Publishing branches** (nightly, dev, gh-pages, release/**) are merges in effect: a workflow
  that pushes a build to them, or a branch whose push publishes, is a merge by another name. They
  must be **in the protected pattern** (section 2.4, 8); the status lists the repo's publishing
  branches and treats any not in the pattern as a gap.

What the lock check reads (7): the repo's **Actions default token permission** (read vs
read/write) and "can Actions approve pull requests"; **environment branch rules** (which branches
can deploy to which environment) and required reviewers; **deploy keys** (a write deploy key is a
bypass-capable credential on the ruleset list as `DeployKey`, [doc: rules page]); and **whether any
workflow triggers on `push` or `pull_request` for an unprotected branch** with a secret or a write
`GITHUB_TOKEN` (read from `.github/workflows` of the default branch, text, best effort).
Setting the Actions default to read, the environment branch rules to the protected patterns, and
moving secrets to protected environments are the fixes the status names. **Accepted residual:** a
chat can still edit code a trusted workflow later runs on a *protected* branch only through a
merge, which the lock stops; on **unprotected** branches a workflow running the chat's edits with
only a read `GITHUB_TOKEN` and no secrets is accepted as the price of letting a chat push.

**GitLab is wider** (section 3): `.gitlab-ci.yml` on the chat's branch runs with the project's
unprotected variables and runners that take unprotected refs, and the chat can edit the file
itself with Developer. The check reads variable protection and runner protection **[verify]**; the
design for GitLab names this route **open** until slice 9's read finds it closed.

## 7. Reading the lock

### 7.1 One answer, with the merge ask's

The repo already has a read of "does a ruleset protect this base":
`ruleset_protects` (`crates/purlis-core/src/forge/github/mod.rs:758`, GitLab
`forge/gitlab/mod.rs:720`), classified as `Ruleset::{Protects, None, Unknown(String)}`
(`crates/purlis-core/src/mergehold.rs:567`). The lock status **extends this read, it does not
sit beside it**: one function over the forge's answer returns the lock state, and the merge ask's
`Ruleset` is a projection of it (Protects when the branch is locked or has the named gaps that do
not touch merging, None, Unknown). Two readers of the same fact that disagree is a defect, so the
contract test (`forge_contract.rs`, the `ruleset_protects` rows) gains the lock rows and a test
fails if the two projections differ on a recorded answer.

### 7.2 States and the store

The check answers per repo: **locked**, **locked except** (named accepted gaps), **not locked**
(named gaps), **cannot read** (needs admin, or the login lacks the scope: section 9). Gaps it
names:

1. Enforcement is not *Active* (disabled or evaluate) [doc: rules page, creating page].
2. The ruleset's **exclude** conditions remove a protected branch or tag from its target
   (`conditions.ref_name.exclude`, **[verify]** the field).
3. The bypass list holds anything but the person: `Team`, `Integration` (the chat's App, or any
   other App), `DeployKey`, `OrganizationAdmin`, a `RepositoryRole` broader than admin, a custom
   role, or `bypass_mode: exempt` (no rules, no audit entry [doc: rules page]).
4. **Custom roles** based on write or maintain on the bypass list or granting bypass
   [doc: creating page, eligible list].
5. **Org and repo rulesets layered**: the answer is read as the union at the branch ("get rules
   for a branch" returns active rules at every level [doc: rules page]), and the bypass lists of
   *all* rulesets that touch the ref are read; one organisation ruleset with a wider bypass list
   does not weaken a stricter repository one, because the most restrictive rule wins
   [doc: about-rulesets] **[verify]** whether a bypass actor on one ruleset exempts the actor from
   the others (row B4).
6. **`~ALL` and `~DEFAULT_BRANCH`** in `include` are expanded: `~DEFAULT_BRANCH` follows a default
   branch change; the check compares the list of **publishing and protected branches** with the
   patterns, not only the literal text.
7. **Classic branch protection** (rules that are not rulesets) is read as well: it can protect a
   branch the rulesets do not, and its "allow specified actors to bypass" list is another bypass
   path. The two are combined by the forge [doc: about-rulesets says rulesets layer with branch
   protection, **[verify]** the exact words]; the check reads both and lists classic protection's
   bypassers.
8. **Auto-merge on, merge queue required** (6.3).
9. **A publishing branch is not in the protected pattern** (6.4).
10. **Actions default write, environments without branch rules, a write deploy key** (6.4).
11. **Immutable releases off** (6.2): a gap marked "accepted" by default because it is a layer.
12. **No path to the person's login** is not proven for a harness (6.1): the strongest gap; the
    state cannot be *locked* with it.
13. **Plan cannot lock** (free private repo, section 9).

**The acceptance and the lock cache live in a store no chat writes**, set only by a window press.
A new store names its tier: **Machine, syncable**: not the plane, so a chat cannot write it
through the plane folder; the cached lock answer is **Machine, rebuildable**; the per-repo
acceptance is **Machine, device-bound** (an acceptance is the person's on this machine). Both go in
`docs/plane-format.md` under a `**Tier:**` line, which
`crates/purlis-core/tests/it/every_store_the_plane_format_names_has_a_tier.rs` enforces. Writes
take the host's scope (as the forge-account store does, `forges.md`: "Only the window signs in
or out") which a chat's process is refused; test: `a_lock_acceptance_never_reaches_a_chat`, the
shape of `a_signed_in_forge_token_never_reaches_a_chat.rs`. The acceptance records the gaps it
accepted, the repo, the date and the person's press; a **new gap invalidates it** (an acceptance
of gaps A and B does not cover C).

### 7.3 A check that needs no admin

Reading rulesets with their bypass lists needs admin or write access to the ruleset
[doc: rules page, "returns bypass_actors only to a caller with write access to the ruleset"].
Two reads need less:

- **"Get rules for a branch"** works with an installation token holding **Metadata: read** and
  returns the active rules at every level [doc: rules page]: so any chat's token can say "the
  default branch has Restrict updates, Restrict creations…" with no admin. It does not say who
  can bypass.
- **`current_user_can_bypass` with the chat's token**: the one question that matters, "can **this**
  credential bypass", answered by the forge for the chat's own token, with no admin and no list
  parsing. The field was not found on the page read (2.14): **[verify]** (row C1). If it exists
  and returns what its name says, the **mint** runs it with the token it just minted, against
  the default branch and a publishing branch, and refuses to hand over a token that can bypass.
  That is the most robust check because it asks the forge, not our reading of the list. Without
  it, the admin read of 7.2 is the fallback.

### 7.4 Every repo, every mint, live tokens

The mint covers **every repo of the workspace** the installation reaches
(`apptoken.rs:560-566` collects `repositories[]` from every remote of the installation), so the
lock is checked on **each of them at each mint**, and one unlocked repo makes the mint either
narrow to the locked repos (`repositories[]` shrinks, **recommended**, with the narrowing said) or
be refused. Tokens are cached for an hour (the App token's life, `expires_at`); if the lock **weakens**
while a token lives (a ruleset disabled, the bypass list edited, auto-merge turned on), the
nightly or the next read finds it and the app calls **`revoke_now`**
(`apptoken.rs:1094`) on the live tokens of the repo's chats and says so. Between the weakening and
the next read the window is the read interval, **accepted**: a webhook on `repository_ruleset`
events would shorten it and is out of scope (the App subscribes to no events today).

## 8. Exact forge setup

### GitHub (per repo; an organisation ruleset where the person owns the org)

Needs a repository admin; org rulesets need an org owner on Team or Enterprise (section 9).

1. **Install the App on the repos only.** Keep **Administration** off; Workflows only as 6.4 says.
2. **Branches ruleset** `purlis: protected branches`: enforcement *Active*; targets: the default
   branch (`~DEFAULT_BRANCH`), the person's patterns, **and every publishing branch** (nightly,
   dev, gh-pages, `release/**`); no exclude; rules: Restrict creations, updates, deletions, Block
   force pushes; bypass: **the person**, mode `pull_request` (decision 13.1).
3. **Tags ruleset** `purlis: tags`: target *Tag*, `**`; Restrict creations, updates, deletions;
   bypass: the person, mode `always`.
4. **Repository settings**, read and shown, set only by the person's press: Allow auto-merge off;
   no merge queue unless the person runs one (then accepted, 6.3); **Immutable releases on**;
   **Actions default token read**; environment branch rules; no write deploy key.
5. **Optional gate** (only if slice 0 shows a non-bypass merge passes, row M2, or the person runs
   auto-merge): a second private App `purlis-gate`, and a required status check on the branches
   ruleset with that App's `integration_id`.
6. The same through the API, for the app's set action: `POST /repos/{o}/{r}/rulesets`
   [doc: rules page].

### GitLab: as round 1 (protected branches and tags to Maintainers or No one, a Developer-role
project token), plus the CI settings of 6.4: protected variables for anything secret, protected
runners, and `.gitlab-ci.yml` changes only on protected refs **[verify]** how.

## 9. What it costs the person, and who cannot set it

- **Non-admin collaborators lose merge and push.** On the protected patterns only the bypass
  actor can merge or push; teammates who merge today cannot, unless they are on the bypass list
  (which widens the lock to them: a chat holding their credential bypasses too). Dependabot and CI
  that **tag, push to a publishing branch or release** (a release workflow, a nightly) stop,
  unless the bypass list names them (and then they are an actor, 7.2 gap 3). Dependabot is an
  eligible bypass actor [doc: creating page].
- **A non-admin person can neither set nor read the lock.** The page for them: a short "send this
  to your repo's admin" screen with the ruleset JSON, the settings list, and what the status will
  say once set. State: *cannot read*; the mint is not refused (they cannot fix it) but the
  status says "unlocked or unknown" and the person's acceptance is theirs to record.
- **Free private repos cannot be locked** (rulesets: public repos on Free; private on Pro/Team/
  Enterprise [doc: about-rulesets]). State: *plan cannot lock*; refuse or warn per 13.2.
- **Personal vs org repos.** `OrganizationAdmin` is not applicable to personal repos
  [doc: rules page]; the person's role there is the repo's owner. Org rulesets need Team or
  Enterprise [doc: creating page].
- **The app's login may not be able to set the lock.** The app signs in with an OAuth token of
  scopes `repo` and `read:org`, no `admin:org` (`forges.md:353-385`), or a fine-grained token.
  Creating a repository ruleset needs repository administration; creating an **organisation**
  ruleset needs org administration the OAuth login does not have, so **organisation rulesets are
  not buildable today** and the doc offers them only as a setup the person does by hand. Whether
  the `repo` scope suffices for `POST /rulesets` on a repo the person administers is **[verify]**
  (row S1).
- **Gaps never silently pass**: *cannot read* is a state, shown, never read as *locked*, as
  `Ruleset::Unknown` is never read as none (`mergehold.rs:567`).

## 10. Slices

### 10.1 The order

Each is a pull request of its own and names the test that goes red.

0. **Verify script** (11): a scripted pass on a throwaway repo; answers recorded in
   `docs/research/`. **Red:** a recorded answer that disagrees with a design assumption fails the
   `forge-lock-answers` test, which reads the research file's table.
1. **No path to the person's login.** The probe of 6.1 per harness; ADR 0067 amended; the forge
   credential class enforced for a chat on a repo that claims the lock; no `gh` fallback.
   **Red:** `a_chat_with_no_mint_has_no_forge_credential` (the probe's `gh api user` returns the
   person's login today), run for Claude Code, Codex and OpenCode.
2. **Narrow the mint.** `mint_over` sends `permissions` as an **exact allowlist**: `contents`,
   `issues`, `metadata`, `pull_requests` (and `workflows` only where the App has it and the
   person ticked it); never `administration`, `actions`, `statuses`, `deployments`, anything else.
   It goes through `Field::text("permissions[contents]", "write")`: `forge/http.rs:11-13, 544-555`
   builds `a[b]` as key `b` of object `a`, so the nested object is already supported by the native
   transport; the contract is that `gh api` builds it the same way (`http.rs:901`). **Red:**
   `the_mint_body_is_exactly_the_allowlist` against the fake forge, and a test that a permission
   the App later gains is absent from the chat's token.
3. **Read the lock** (gap states, 7), by extending `ruleset_protects` (7.1), with the no-admin
   reads (7.3). **Red:** a recorded ruleset with an excluded branch, an `Integration` bypass, an
   `exempt` mode, a disabled enforcement, a missing publishing branch each reads *not locked*; and
   `the_merge_ask_and_the_lock_status_agree`.
4. **Show it, warn only.** Settings › Project row per repo (a `Notice`, with the fix or a link to
   the setup page); pane head says when a chat's repo is unlocked; the mint warns and proceeds.
   Changelog fragment. **Red:** `Notice.guard.test.ts` and a copy test for the sentences; the
   status is absent for a repo with no chat.
5. **Set it per repo**, with the **non-admin and plan-refusal paths**: one press creates the
   rulesets with the person's login, shows the JSON first, reads back to confirm; a non-admin gets
   the "send to your admin" page; a plan that cannot lock gets its refusal sentence. **Red:**
   `the_set_lock_reads_back_what_it_wrote` and `a_non_admin_sees_the_admin_page`, against the fake.
6. **Refuse an unlocked mint, with migration** (10.2). **Red:** `an_unlocked_repo_refuses_a_mint`
   and `a_recorded_acceptance_lets_it_mint`.
7. **Tag as the app's act** (10.3). **Red:** `a_chat_sent_tag_push_never_takes_the_app_path`.
8. **Release as the app's act** (10.3). **Red:** `a_chat_never_creates_the_release_the_app_made`.
9. **GitLab read, show, set**: protected branch/tag, CI variable/runner reads; same states as 7.
   **Red:** the GitLab rows of the contract test.
10. **GitLab chat credential**: a new feature: a Developer-role project token minted or kept in a
    vault, the brokered exec, a status. **Red:** `a_gitlab_chat_has_a_developer_token_never_the_persons`.
11. **Nightly live run** of the test plan (12) against a throwaway repo and project.
    **Red:** the nightly's report when a recorded answer changes.

The **gate App** is built only if slice 0 shows a non-bypass merge passes (row M2), or as the
auto-merge mitigation (6.3) if the person wants auto-merge.

### 10.2 Migration

**Warn-only for a release, then refuse, with a per-repo recorded acceptance.** Release N: slice 4
shows the status and the mint **warns once per repo per day** and proceeds; the Notice has the fix
and "Accept this gap for this repo". Release N+1: slice 6 refuses a mint for a repo that is not
*locked* and has no acceptance covering **all its current gaps**; the refusal says what to do (set
the lock, or accept). Acceptances (7.2) are per repo, name the accepted gaps, and are invalidated
by a new gap. Existing chats keep their live tokens until the hour ends; the refusal applies to
the **next** mint. A person who never wants the lock accepts every repo once.

### 10.3 Tag and release as the app's act (the seam is not there today)

The release floor (`floorguard.rs`) only reads text; there is **no release or tag seam** in the
app, so slices 7 and 8 build one. Design, in the shape of the merge hold:

- **The ask** is an act of the chat's request (`purlis release ask`, like the merge ask) carrying
  `{repo, tag, target commit sha, notes, asset list}`; the app **binds the act to the commit** (the
  full sha, not a branch name) exactly as the merge hold binds a merge to a head, and the press
  creates the tag and the release as the person, **for that sha only**. A moved branch does not
  move the act.
- **Build assets.** The build is the chat's or CI's; the assets reach the release by the app
  reading them from the chat's worktree path or from the CI run's artifact (a named workflow run
  id) and uploading them with the person's login after the press, listing their sha-256 in the
  confirmation. **[verify]** that a release can be created as draft, assets attached, and then
  published in one session of the person's token (the immutable-releases recommended flow
  [doc: immutable-releases page]); that is the app's flow: draft, upload, publish.
- **Signed tags.** The chat cannot sign (no key); the app creates an **annotated tag object**
  (`POST /git/tags`, which returns 201 and writes an object, row 1) and the ref (`POST /git/refs`),
  both as the person; **whether GitHub shows the tag as verified** with the person's GPG/SSH key
  depends on a signing key the app does not hold: **[verify]** and a **named residual**: a signed
  tag by the person stays the person's `git tag -s` and `git push`, outside the chat. The
  alternative, a lightweight tag + release via the API, is recorded as "not signed".
- **Q330** (a ref update of the default or protected branch is held as a merge) already covers the
  branch half; the tag half is new.
- **GitLab:** a release by the person on a protected tag; GitLab's release API is Developer-level
  [doc: releases page] so the tag protection is the lock there.

## 11. Slice 0: the verify run, first

Slice 0 is the **first** slice, not the last: every decision in 5 depends on rows below that no
page answers. **Not run by this task**: the person decides (13.3) and creates the repo; this
section is the script and what is expected.

**Setup (the person, once).** A throwaway private or public repo `purlis-lock-probe` under their
account (public needs no paid plan for rulesets; **a private repo needs Pro or above**), `main` with
one commit, `release` and `nightly` branches, a tag `v0`, a release `v0`, a workflow
`ci.yml` on `push` and `pull_request`, and the App installed on it with exactly the manifest
permissions. Two credentials: `ADMIN` (the person's login) and `CHAT` (a token minted by the real
mint for the repo; for rows with a narrowed token, mint one with the allowlist of slice 2).

```sh
#!/usr/bin/env bash
# forge-lock-probe.sh: write each answer to answers.tsv (row, expected, HTTP status, body head).
# Needs: gh, jq. REPO=owner/purlis-lock-probe ADMIN=<token> CHAT=<token>. Run by the person.
set -u
api() { # api <token> <method> <path> [json]   -> prints "<status> <body head>"
  local t=$1 m=$2 p=$3 d=${4:-}
  curl -s -o /tmp/p.out -w '%{http_code}' -X "$m" -H "Authorization: Bearer $t" \
    -H "Accept: application/vnd.github+json" ${d:+-d "$d"} "https://api.github.com/repos/$REPO$p"
  echo " $(head -c 160 /tmp/p.out | tr '\n' ' ')"
}
row() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" | tee -a answers.tsv; }
SHA=$(gh api repos/$REPO/commits/main --jq .sha)

# Phase A: no ruleset. Everything should SUCCEED as CHAT: this proves the tests can fail.
row M0 "expect 200 (merge by contents:write)" "$(api $CHAT PUT /pulls/1/merge)"
# Phase B: apply rulesets of section 8 (branches: ~DEFAULT_BRANCH, release, nightly; tags: **),
# bypass = ADMIN only, mode always. Then run every row as CHAT, then as ADMIN.
row M1 "expect 403/422 (merge refused)"          "$(api $CHAT PUT /pulls/2/merge)"
row M2 "DECIDES the gate App: 403 means no gate"  "$(api $CHAT PUT /pulls/3/merge)"
row M3 "merge-async (unconfirmed endpoint)"       "$(api $CHAT PUT /pulls/4/merge-async)"
row P1 "PUT /contents on main"        "$(api $CHAT PUT /contents/x.txt '{"message":"m","content":"eA==","branch":"main"}')"
row P2 "POST /merges into main"       "$(api $CHAT POST /merges '{"base":"main","head":"feature"}')"
row P3 "PATCH refs/heads/main"        "$(api $CHAT PATCH /git/refs/heads/main "{\"sha\":\"$SHA\",\"force\":true}")"
row P4 "DELETE refs/heads/release"    "$(api $CHAT DELETE /git/refs/heads/release)"
row P5 "POST /git/refs nightly"       "$(api $CHAT POST /git/refs "{\"ref\":\"refs/heads/nightly2\",\"sha\":\"$SHA\"}")"
row P6 "rename feature -> release/x"  "$(api $CHAT POST /branches/feature/rename '{"new_name":"release/x"}')"
row P7 "merge-upstream (fork)"        "$(api $CHAT POST /merge-upstream '{"branch":"main"}')"
row P8 "PR base retarget to main"     "$(api $CHAT PATCH /pulls/5 '{"base":"main"}')"
row P9 "GraphQL createCommitOnBranch main (see below)" "(run with gh api graphql)"
row P10 "GraphQL mergeBranch (see below)"               "(run with gh api graphql)"
row T0 "POST /git/tags (object only)  expect 201"      "$(api $CHAT POST /git/tags "{\"tag\":\"v9\",\"message\":\"m\",\"object\":\"$SHA\",\"type\":\"commit\"}")"
row T1 "POST refs/tags/v9 (expect 403/422)"            "$(api $CHAT POST /git/refs "{\"ref\":\"refs/tags/v9\",\"sha\":\"$SHA\"}")"
row T2 "PATCH refs/tags/v0 (expect refused)"           "$(api $CHAT PATCH /git/refs/tags/v0 "{\"sha\":\"$SHA\",\"force\":true}")"
row R1 "POST /releases NEW tag v10 (expect refused when tag creation restricted)" "$(api $CHAT POST /releases '{"tag_name":"v10","target_commitish":"main"}')"
row R2 "POST /releases on EXISTING tag v0 (design says SUCCEEDS)" "$(api $CHAT POST /releases '{"tag_name":"v0"}')"
row R3 "POST /releases draft:true (design says succeeds)" "$(api $CHAT POST /releases '{"tag_name":"v0","draft":true}')"
row R4 "PATCH release draft:false (publish)"            "$(api $CHAT PATCH /releases/<id> '{"draft":false}')"
row R5 "with immutable releases on: PATCH asset / DELETE release / re-create" "(manual, see below)"
row R6 "chat edits a published immutable release's body" "$(api $CHAT PATCH /releases/<id> '{"body":"x"}')"
row L1 "PUT /rulesets/<id> as CHAT (disable the lock)"   "$(api $CHAT PUT /rulesets/<id> '{"enforcement":"disabled"}')"
row L2 "DELETE /rulesets/<id> as CHAT"                  "$(api $CHAT DELETE /rulesets/<id>)"
row L3 "PATCH repo allow_auto_merge true as CHAT"       "$(api $CHAT PATCH '' '{"allow_auto_merge":true}')"
row C1 "current_user_can_bypass with CHAT: GET /rulesets/<id>?includes_parents=true" "$(api $CHAT GET /rulesets/<id>)"
row C2 "GET /rules/branches/main with a Metadata-only token" "$(api $CHAT GET /rules/branches/main)"
row B1 "narrowed token (permissions=contents:write only) of a bypass App still bypasses?" "(Phase C)"
row B3 "ruleset bypass actor_type=User accepted? (as ADMIN)" "(POST /rulesets, see below)"
row B2 "ADMIN in mode pull_request: direct push refused, PR merge works" "(as ADMIN)"
row A1 "CHAT enables auto-merge (GraphQL enablePullRequestAutoMerge); merge made by whom" "(see below)"
row Q1 "merge queue: CHAT adds to queue under Restrict updates" "(see below)"
row W1 "push a branch editing scripts/run.sh; ci.yml runs; GITHUB_TOKEN permission? repository_dispatch as CHAT?" "(see below)"
row S1 "OAuth login (repo scope) can POST /rulesets" "(as the app's login)"
# GraphQL rows run with: gh api graphql -f query=@q.graphql, with GH_TOKEN=$CHAT, then $ADMIN.
```

**Expected answers (hypotheses, recorded as observed, never as decided):**

| Row | Expected as CHAT | Expected as ADMIN | If it differs |
|---|---|---|---|
| M0 (no ruleset) | succeeds | succeeds | the test cannot fail: fix the probe |
| M1 merge, M2, P1 to P8 on a protected ref | refused (403 or 422), ref unchanged | succeeds | **M2 passes: build the gate App** (13.1 alt.); P-row passes: that route is a gap, listed |
| P9, P10 GraphQL | refused | succeeds | as P rows |
| M3 `merge-async` | the endpoint **may not exist** | n/a | drop it from the doc |
| T0 `POST /git/tags` | **201** (object, no ref) | 201 | fix section 1 |
| T1, T2 tag ref | refused | succeeds | tag ruleset incomplete |
| R1 release, **new** tag | refused (tag cannot be created) | succeeds | note what `target_commitish` does |
| R2 release, **existing** tag | **succeeds** | succeeds | if refused, 6.2 loses its main gap |
| R3, R4 draft, publish | **succeeds** | succeeds | the "locked except releases" gap stands |
| R5, R6 immutable on | asset change and delete refused; notes edit allowed | same | else 6.2 option 1 is weaker |
| L1, L2 ruleset change as CHAT | **refused** (needs Administration) | succeeds | **critical**: the chat can disable the lock |
| L3 allow auto-merge as CHAT | refused (Administration) | succeeds | gap 8 is a chat route |
| C1 | answer shows whether the token can bypass | | if no field: drop 7.3 first bullet |
| C2 | the active rules at the branch | | if refused: the no-admin read is not available |
| B1 | still bypasses (same actor) | | else "two tokens" of one App works |
| B2 | n/a | direct push refused, PR merge succeeds | else `pull_request` mode is not recommended |
| B3 | n/a | accepted or refused | decides "the person only" vs a role |
| A1 | enable permitted or refused; merge attributed to X | | decides gap 8 and the gate |
| Q1, W1, S1 | observed | | decides 6.3, 6.4 and section 9 |

Run **every row first as CHAT, then as ADMIN**: the second proves the first was refused for the
right reason. Run phase C (bypass actor Integration for the App and a narrowed token) last and
remove it. The answers are committed as a table in `docs/research/forge-lock-verify-results.md`, recorded by the kit of 13.3,
with the date, the plan of the account and the GitHub API version header, and the nightly (slice
11) reruns the same script.

### 11.1 The claims this settles

`/merge-async` exists (M3); `actor_type: User` is a usable bypass (B3); `bypass_mode: exempt`
leaves no audit entry [doc: rules page] and whether the chat's token matters there (not a design
need: the status lists `exempt` as a gap); `current_user_can_bypass` exists (C1); the
non-admin read (C2); immutable releases (R5, R6).

## 12. Test plan

Two layers: **a live forge** says whether the forge refuses; **a fake forge** says whether purlis
uses the right credential and reads the answers right.

**Layer 1, live (slice 11).** Each row is made as the chat's real mint and must be refused with the
ref unchanged, then repeated as the person and must succeed; and "**the person's press still
merges**" is a row of its own (the merge hold's native-transport merge, under the ruleset, in
bypass mode `pull_request`).

### 12.1 Branch and ref rows

| Effect | Made through |
|---|---|
| Merge a pull request | `gh pr merge`, REST `PUT /merge`, GraphQL `mergePullRequest` |
| Write a file | `PUT /contents` on a protected branch |
| Commit to a branch | GraphQL `createCommitOnBranch` |
| Merge a branch | GraphQL `mergeBranch`; REST `POST /merges`; `merge-upstream` |
| Update, delete, create a protected ref | `git push`, `PATCH/DELETE/POST /git/refs`, GraphQL `updateRef`, `createRef`, `deleteRef` |
| **Retarget a pull request's base** | `PATCH /pulls/{n}` `base` to a protected branch |
| **Rename a branch into a protected pattern** | `POST /branches/{b}/rename` to `release/x` |
| Rename or change the default branch | `rename`, `PATCH /repos` |
| **Change the lock** | **ruleset `PUT` and `DELETE` as the chat token**; `PATCH /repos` for `allow_auto_merge` |
| Merge later | `enablePullRequestAutoMerge`; add to the merge queue |

### 12.2 Release and tag rows

`POST /git/tags` (201, object only); `POST /git/refs` with `refs/tags/…`; `git push` of a tag;
**`POST /releases` on an existing tag (succeeds today)**; `POST /releases` on a new tag
(refused); draft create, draft publish, edit, delete, asset upload and replace; with immutable
releases on: asset change refused, delete refused, notes edit allowed.

### 12.3 CI rows

Push a branch editing a script an existing workflow runs: does the run get a write
`GITHUB_TOKEN` and secrets; `repository_dispatch` as the chat; `workflow_dispatch` by a job
with `actions:write`; with `Wanted::workflows` on, a pushed workflow file; an environment with
branch rules refusing the chat's branch; a write deploy key. GitLab: the same with `.gitlab-ci.yml`,
unprotected variables and runners.

### 12.4 The allowed rows and the person

A chat pushes a feature branch, opens and comments on a pull request, edits its own branch: still
succeed. **The person's press still merges**; the person's tag and release act succeed.

**Layer 2, a fake forge.** A local server answering what the live run recorded: the mint's body is
exactly the allowlist (slice 2); each recorded ruleset classifies into its gap (7.2), including
disabled enforcement, an excluded branch, `Integration`/`Team`/`DeployKey`/`OrganizationAdmin`/
`exempt` bypass, `~ALL`, a classic protection with bypassers, a layered organisation ruleset,
auto-merge on, a queue, a missing publishing branch; the merge ask's `Ruleset` and the lock
agree (7.1); an unlocked repo refuses a mint after migration; the acceptance store is never
written by a chat; the probe of 6.1 fails with a person's `gh` login present. Each assertion is
first run against a ruleset missing that rule, per the repo's rule that a test is trusted once
seen to fail.

## 13. Open decisions for the person

**Decided** by the person, on issue #7133 (2026-11-18). Slice 0 still measures what these rest on
(B3 for the first); where it disagrees, the row says so and the decision is reopened.

1. **The bypass actor and its mode: decided.** Bypass is **the person alone**: `actor_type: User`
   if GitHub accepts it (row B3), else the **repository admin role**. Mode **`pull_request`** on
   branch rulesets and **`always`** on tag rulesets. The gate App is built only if row M2 or
   auto-merge makes it necessary.
2. **Refuse vs warn: decided.** An unlocked repo **warns, then refuses** (after one release of
   warning, 10.2). A repo that cannot be locked (free private, no admin, GitHub Enterprise Server,
   GitLab before slice 10) warns and records the person's acceptance.
3. **Slice 0 runs on a throwaway repo: decided.** A **public** repo under the person's account
   (public needs no paid plan for rulesets), named `purlis-forge-lock-verify-*`, **deleted
   afterwards**. The kit is `tools/forge-lock-verify/`: `person.sh` (every step that acts as the
   person: create the repo, install the App, both rulesets and the bypass rows, the person's press,
   auto-merge, delete) and `chat.sh` (every row as the chat's minted token); each row is recorded
   as row, expected, observed, pass/fail in `docs/research/forge-lock-verify-results.md`. A chat
   never uses the person's credentials; this task wrote the kit and ran nothing against a forge.

Smaller, with a default: ADR 0067's amendment and a new ADR for the lock (default: an amendment
of 0067 and 0077 each, `docs/adr/<NNNN>/<date>-<slug>.md`); GitHub Enterprise Server is out of
scope (mint is github.com only, `apptoken.rs`); GitLab without Premium takes a Developer user's
token; org vs repo rulesets: repo only (section 9: not buildable at org level today).

## 14. Residuals, accepted, with the reason

- A credential the person keeps where purlis does not look, and a connector it cannot name (6.1).
- Drafts and the notes of published releases (6.2): reversible, not what users download.
- Edits a trusted workflow runs on an unprotected branch with a read-only token and no secrets
  (6.4): the price of letting a chat push.
- The interval between a lock weakening and the next read, until `revoke_now` (7.4).
- A signed tag is the person's own act (10.3).
- A person on a free private repo, a non-admin, GitHub Enterprise Server (section 9).

## 15. Sources read

GitHub (round 1 and 2; all 2026-11-18):
- <https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps>
- <https://docs.github.com/en/rest/apps/apps>
- <https://docs.github.com/en/rest/repos/rules>
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets>
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository>
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets>
- <https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/configuring-tag-protection-rules>
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/renaming-a-branch>
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/changing-the-default-branch>
- <https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository>
- <https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases> (round 2)
- <https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request> (round 2)
- <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue> (round 2)

GitLab (round 1):
- <https://docs.gitlab.com/user/project/repository/branches/protected/>
- <https://docs.gitlab.com/user/project/protected_tags/>
- <https://docs.gitlab.com/user/permissions/>
- <https://docs.gitlab.com/user/project/releases/>
- <https://docs.gitlab.com/user/project/repository/branches/default/>
- <https://docs.gitlab.com/user/project/settings/project_access_tokens/>

**Not read, so every claim that needs them is [verify]:** "Preventing changes to your releases"
(did not load; who enables immutable releases), GitHub Actions' default token permissions,
environments and deploy keys, classic branch protection, `current_user_can_bypass` (not on the
rules REST page), custom repository roles; GitLab's protected variables, protected runners and
CI pages. Slice 0 and the first read of slice 9 close them.
