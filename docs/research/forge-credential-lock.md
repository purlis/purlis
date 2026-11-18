# The merge and release lock is the forge credential (#7133, Q331)

Status: design for the person to grill, then a Fable review. No implementation code.
Date: 2026-11-18. Rulings it rests on: Q331 (the lock is the credential; the shell-text guard
stays as the early warning), Q330 (an API ref update of the default or protected branch is held
as a merge). Related: #7092, #7124, #7131, #7118.

**How to read the facts.** Every forge fact below is marked **[doc]** with the page it was read
on (2026-11-18, docs.github.com and docs.gitlab.com only), **[inferred]** (follows from a
**[doc]** fact, but no page says it in so many words) or **[verify]** (no page read answered it;
the test plan in section 7 settles it on a test repo before anything ships). Nothing marked
**[verify]** is a basis for a decision yet.

## 1. What a chat's credential can do today

From the repo:

- The App `purlis-purlis-agent` is made from a manifest asking `contents:write`, `issues:write`,
  `metadata:read`, `pull_requests:write`, and `workflows:write` only if the person ticked it
  (`crates/purlis-core/src/forge/agentidentity.rs`, `Wanted::permissions`).
- A chat's token is minted by the app per chat and clone: the App's JWT, then
  `POST /app/installations/{id}/access_tokens` with `repositories[]` and **nothing else**
  (`crates/purlis-core/src/forge/apptoken.rs`, `mint_over`). It therefore carries every
  permission the installation has. The App's key is in vault `agent-identity` under a reserved
  prefix no read but the mint's gets (`sandbox/tools/mint.rs`); the chat gets only the token, in
  `GH_TOKEN`, through the brokered `secret exec` (`secrets/brokered.rs`).
- The merge hold (`mergehold.rs`, `app/src-tauri/src/mergeholds.rs`) merges with **the person's
  own forge login** in the window (the native transport), never with the App's token. The
  release floor (`floorguard.rs`) only reads command text.

What GitHub says that token can do **[doc]**
(<https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps>, each row
read from the table of the named permission, "write" with installation tokens allowed):

| Effect | Endpoint | Permission it needs |
|---|---|---|
| Merge a pull request | `PUT /repos/{o}/{r}/pulls/{n}/merge` (and `/merge-async`) | **Contents: write** (not Pull requests) |
| Merge a branch | `POST /repos/{o}/{r}/merges` | Contents: write |
| Create, update, delete a ref (branch or tag) | `POST/PATCH/DELETE /git/refs[/{ref}]` | Contents: write |
| Create a tag object | `POST /git/tags` | Contents: write |
| Create or edit or delete a release | `POST /releases`, `PATCH/DELETE /releases/{id}` | Contents: write |
| Rename a branch | `POST /branches/{b}/rename` | Contents: write |
| Change the default branch | `PATCH /repos/{o}/{r}` | **Administration: write** |

So `contents:write`, which a chat needs to push its own branch, is also the permission that
merges, tags, releases and renames. The App does **not** hold Administration, so changing the
default branch is already refused to its tokens **[inferred]**; the other five are not.

## 2. What GitHub can narrow, and what it cannot

1. **A token can be narrowed per request, downward only [doc].** The access-token call takes
   `repositories` or `repository_ids` (at most 500) and a `permissions` object; without
   `permissions` the token gets every permission the App was granted, and it "cannot be granted
   permissions that the app was not granted"
   (<https://docs.github.com/en/rest/apps/apps>, "Create an installation access token for an
   app"; the call needs the App's JWT).
2. **Narrowing cannot split push from merge.** Pushing a branch and merging a pull request are
   both `contents:write` (section 1), so any token that can push can merge, tag and release
   **[inferred]**. `contents:read` would stop all of it, including the chat's own pushes.
3. **A ruleset's bypass list names actors, not tokens [doc].** `bypass_actors[]` takes
   `actor_type` of `Integration`, `OrganizationAdmin`, `RepositoryRole`, `Team`, `DeployKey` or
   `User`, an `actor_id`, and a `bypass_mode` of `always`, `pull_request` (branch rulesets only)
   or `exempt`
   (<https://docs.github.com/en/rest/repos/rules>; the creating page lists roles, teams and
   GitHub Apps: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository>).
   An `Integration` is the App; so **two tokens of one App, with different `permissions`, are the
   same actor [inferred]**, and the "two tokens" option cannot work with one App. Two Apps are two
   actors **[inferred]**. **[verify]** that a narrowed token of a bypass App still bypasses.
4. **The rules that matter [doc]**
   (<https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets>):
   *Restrict creations* ("only users with bypass permissions can create branches or tags whose
   name matches"), *Restrict updates* (only bypass actors "can push to" matching branches or
   tags), *Restrict deletions*, *Block force pushes*, *Require a pull request before merging*,
   *Require status checks*, whose entries can name an `integration_id` that the check must come
   from (<https://docs.github.com/en/rest/repos/rules>). Rulesets target branches **and tags**,
   by fnmatch pattern; several rulesets are aggregated, the most restrictive version of a rule
   wins (<https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets>).
5. **Rulesets need a plan [doc]** (same page): public repos on Free; public and private on Pro,
   Team and Enterprise Cloud. Push rulesets (not needed here) are Team and up.
6. **Legacy tag protection is gone as a separate feature [doc].** Its page now redirects to the
   rulesets page
   (<https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/configuring-tag-protection-rules>),
   so tags are protected with a ruleset whose target is "tag".
7. **Rename and default branch [doc].** Anyone with write can rename a branch "unless it is the
   default branch or covered by a branch protection or ruleset"; admins rename the default and
   protected branches
   (<https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/renaming-a-branch>).
   Changing the default branch needs admin
   (<https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/changing-the-default-branch>).
   So for a chat's write-role token, rename of default or protected is refused by the forge
   **[doc]** with no ruleset needed, but only because the token has no admin role **[verify]**
   for an installation token, whose "role" is its permissions.
8. **Releases [doc].** "Repository collaborators and people with write access … can create, edit,
   and delete a release"
   (<https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository>).
   There is no release permission apart from Contents. A release names a tag; the lock is
   therefore the **tag ruleset** (restrict creations of `refs/tags/**`), not a release rule
   **[inferred]**. **[verify]** that `POST /releases` with a new `tag_name` is refused when the
   tag ruleset forbids the tag's creation, and what `target_commitish` does.
9. **Open on the forge side, none answered by a page read:** whether *Restrict updates* also
   refuses the pull-request merge made by a non-bypass actor (the merge is an update of the
   branch); whether a non-bypass actor can enable **auto-merge**, which merges later with no
   press; whether `GITHUB_TOKEN` of a workflow a chat pushed can write a protected ref. All
   **[verify]**.

## 3. GitLab

- **Protected branches [doc]**
  (<https://docs.gitlab.com/user/project/repository/branches/protected/>). *Allowed to merge*
  controls who merges through merge requests and "create[s] new protected branches"; *Allowed to
  push and merge* controls direct pushes to existing protected branches. Both take roles
  (No one, Maintainers, Developers + Maintainers); groups and single users are Premium and
  Ultimate only. The default branch is protected by default, with Maintainers allowed to push.
  Setting both to **No one** or **Maintainers** leaves a Developer-role token able to push other
  branches and open merge requests but not to merge.
- **Role table [doc]** (<https://docs.gitlab.com/user/permissions/>): *Push to protected
  branches*, *Delete protected branches*, *Manage protected branches* and *Manage protected tags*
  are Maintainer and Owner; creating non-protected branches is Developer.
- **Protected tags [doc]** (<https://docs.gitlab.com/user/project/protected_tags/>): "To create or
  delete a protected tag, you must be in the *Allowed to create* list"; configured by a
  Maintainer or Owner; a wildcard `*` protects every tag; roles are in Free, groups and users in
  Premium.
- **Releases [doc]** (<https://docs.gitlab.com/user/project/releases/>): Developer, Maintainer
  and Owner can create, update and delete releases; "If a release is associated with a protected
  tag, the user must be allowed to create the protected tag too", and the page's own example
  restricts releases to Maintainers by protecting `*` with Maintainer in *Allowed to create*.
- **Default branch [doc]** (<https://docs.gitlab.com/user/project/repository/branches/default/>):
  changing it needs the Maintainer or Owner role.
- **Project access tokens [doc]**
  (<https://docs.gitlab.com/user/project/settings/project_access_tokens/>): a Maintainer or Owner
  creates one, **selects a role for it** and its scopes; it acts as a bot user; on GitLab.com
  they need Premium or Ultimate, on self-managed any licence.
- **Net:** on GitLab the role is the lever GitHub lacks. A chat token with the **Developer**
  role cannot merge, push to, delete or protect the default or a protected branch, cannot create a
  protected tag, and cannot change the default branch, as long as the project's protected
  branch and tag settings name Maintainers (or No one) only. The app acts with the person's own
  login (Maintainer or Owner), as the merge hold already does. **[verify]**: that a Developer
  token is refused on `repository/commits` and `repository/files` writes to a protected branch,
  and on the branch-create API for a protected name.

## 4. Options compared

| | A. Rulesets: the chat's App is never a bypass actor | B. Broker proxying the forge API | C. Two tokens |
|---|---|---|---|
| **Security** | The forge refuses on every path (REST, GraphQL, git protocol, `gh`, `curl`, any language). Holds if rulesets stay as set and the chat never gets the person's login. | Covers only traffic that goes through it. Git over HTTPS and SSH, GraphQL's large surface, and any direct connection must be closed by network policy. A new parser of API calls is the class of tool Q331 says is never complete. | One App's tokens are one actor (section 2.3), so only works with two Apps; then it is A with a second bypass App. With one App and `permissions` narrowing it cannot split push from merge. |
| **Setup the person does** | Once per repo (or once per org with an org ruleset): one ruleset for branches, one for tags, a bypass entry for themselves (admin role). GitLab: protected branch and tag settings plus a Developer token. | A running proxy, and chat network policy that blocks everything else. | Create and install a second App (a GitHub flow the app already runs once for the first). |
| **Cost** | None beyond the plan: rulesets on private repos need Pro, Team or Enterprise Cloud. GitLab roles are Free; project tokens on GitLab.com are Premium. | Code to write and keep correct for every forge endpoint and transport. | A second App key in a vault; the app's mint code twice. |
| **Harness-agnostic** | Yes. The chat holds only a token; no harness hook is involved. | Yes in principle, but each harness's network path needs routing through it. | Yes. |
| **Fails how** | Closed: a route nobody thought of is still refused by the forge. | Open: a route the broker doesn't know is allowed. | As A. |

## 5. Recommendation

**Adopt A on GitHub and the role model on GitLab. Do not build a broker.** In ten lines:

1. The forge, not text, is the lock. The text guard stays as the early warning and its refusal
   says the forge would refuse anyway.
2. GitHub: two rulesets on each repo a chat can reach. *Branches*: the default branch and every
   protected pattern, with Restrict creations, Restrict updates, Restrict deletions, Block force
   pushes. *Tags*: `refs/tags/**`, with Restrict creations, updates and deletions.
3. The bypass list holds the **person's repository admin role** (`RepositoryRole`) and nothing
   else. The chat App `purlis-purlis-agent` is **never** a bypass actor.
4. The app's own act (merge, release, tag, default-branch change) uses the person's login in the
   window, as the merge hold already does. No second App, no second token to guard.
5. The chat's token stays an installation token with `contents:write`: it still pushes its own
   feature branches, which no ruleset covers. Administration and Workflows stay off the App.
6. Every chat token is minted with an explicit `permissions` object (the installation's set
   minus anything unneeded), so a permission added to the App later is not inherited by chats.
   It adds little today and costs one field.
7. GitLab: protected branch and tag settings set to Maintainers (or No one) for *Allowed to
   merge*, *Allowed to push and merge* and *Allowed to create*; the chat gets a **Developer-role**
   project access token; the app acts as the person.
8. The app checks the setup, shows it as a status with a one-press fix, and refuses to hand a
   chat a token for a repo whose lock is not in place unless the person accepted that.
9. A live test repo proves each bypass in section 7 is refused by the forge, and re-runs on a
   schedule, since the forge's behaviour is what this design leans on.
10. Two **[verify]** items can change this: whether *Restrict updates* stops a non-bypass
    pull-request merge, and whether auto-merge can be enabled by the chat. If the first fails,
    add the optional **gate** (section 6, slice 8).

**Why not B.** The broker would have to know every endpoint that changes a ref, and the git
protocol bypasses it. Everything the broker could refuse, the forge can refuse itself, so a
broker is needed for nothing in the five-item list. What the forge cannot refuse
(`curl` to a host the chat's network allows, and reading a secret it was handed) is already the
sandbox's and the vault's job.

## 6. Exact forge setup

### GitHub (per repo, or once as an organisation ruleset)

Needs a repository admin. Org rulesets need the org; use per repo where the person has no org
role. Names are suggestions.

1. **Install the App on the repos only** (already so: the mint is limited to the chat's repos).
   Keep **Administration** and **Workflows** off the App. (Workflows is already opt-in, Q5.)
2. **Branches ruleset** `purlis: protected branches`
   (Settings, Rules, Rulesets, New branch ruleset): enforcement *Active*; target: the default
   branch and any pattern the person lists (for instance `release/**`); rules: Restrict
   creations, Restrict updates, Restrict deletions, Block force pushes; bypass list: the
   **Repository admin** role, mode *Always*. Nobody else, and not the App.
3. **Tags ruleset** `purlis: tags`: target *Tag*, pattern `**` (all tags); rules: Restrict
   creations, Restrict updates, Restrict deletions; same bypass list.
4. Optional **gate** (only if verify item 1 fails): a second private App `purlis-gate` that only
   the app holds, and on the branches ruleset a required status check `purlis/merge-hold` with
   that App's `integration_id`. Nothing merges until the app posts the check, which it does on
   the person's second press.
5. Repository settings to check, not set: *Allow auto-merge* off **[verify]**; merge queue off
   unless the person runs one.
6. The same through the API, for the app's setup action:
   `POST /repos/{o}/{r}/rulesets` with `target`, `enforcement: "active"`, `bypass_actors`,
   `conditions.ref_name.include`, `rules[]` (<https://docs.github.com/en/rest/repos/rules>). The
   response carries `bypass_actors` only to a caller with write access to the ruleset.

### GitLab (per project, or per group for protected branches)

Needs a Maintainer or Owner.

1. Settings, Repository, **Branch rules**: for the default branch and each protected pattern,
   *Allowed to merge* and *Allowed to push and merge* both **Maintainers** (or **No one** for push).
   Allowed to force push off.
2. Settings, Repository, **Protected tags**: wildcard `*`, *Allowed to create* **Maintainers**.
   This also locks releases to Maintainers (the page's own example).
3. Settings, Access tokens: a **project access token**, role **Developer**, scopes `api` (or the
   narrowest the chat's `glab` needs) and `write_repository`, a short expiry, named for the
   chat's persona. It goes in a vault; the brokered `secret exec` hands it to the chat. On
   GitLab.com this needs Premium or Ultimate; otherwise a dedicated Developer user's personal
   access token.
4. Confirm no user, group or deploy key besides Maintainers is listed under any of the above.

## 7. App changes, as slices

Each slice is a pull request of its own, smallest first. None is written by this task.

1. **Read the lock.** A forge call, made with the person's login, that reads the repo's rulesets
   (GitHub) or protected branch and tag settings (GitLab) and answers one of: *locked*, *not
   locked, and why* (named gap), *cannot read* (needs admin). Pure function over the forge's
   answer, tested on recorded answers. A cache of the last answer is a store and names its tier
   in `docs/plane-format.md` (Machine, rebuildable).
2. **Show it.** Settings, Project: a row per repo, a `Notice` with a way out (the fix, or a link
   to the page above). The pane head says when a chat's repo is unlocked.
3. **Set it.** One press creates the two rulesets (GitHub) or the protected settings (GitLab) with
   the person's login, shows the JSON first, and reads the lock again to confirm.
4. **Narrow the mint.** `mint_over` sends a `permissions` object. Test: the request body is
   exactly the expected set, and a permission the App later gains is absent from the chat token.
5. **Refuse an unlocked mint.** A chat is not handed a token for an unlocked repo, unless the
   person has accepted it for that repo (a recorded ruling in the project policy). The refusal
   says what to do.
6. **The release and tag as the app's act.** The release floor's `gh release create` and a tag
   push become the app's own ask with a press, like the merge, and use the person's login
   (Q330 already holds a ref update of the default or protected branch as a merge). The text
   guard is unchanged and now says "the forge refuses this credential regardless".
7. **GitLab mint.** A chat's GitLab credential is a Developer-role project access token in a
   vault, read by the brokered exec, with the lock check of slice 1 for GitLab. There is no
   GitLab App to mint from.
8. **Optional gate.** The second App and the check, only if verify item 1 fails.
9. **The test harness** of section 8, which is also this design's definition of done.

Changelog fragments go with slices 2, 3, 5 and 6 (`changes/<slug>.md`). Docs: a short page in
`docs/` for the setup steps, which section 6 is the draft of.

## 8. Test plan

Two layers, because two things need proving: that **the forge refuses** (only a real forge says
so) and that **purlis uses the right credential** (a fake can say so).

**Layer 1, a live test repo (GitHub) and a test project (GitLab).** A throwaway repo under a
test account with the setup of section 6 applied by slice 3. A chat's credential is minted by the
real mint. Each row below is made as that credential and must be refused (HTTP 403 or 422 on the
API, a rejected push over git) with the ref unchanged; each is then repeated as the person's
login and must succeed, to prove the test is not refused for another reason. The effect is what
the forge sees, so every textual route of #7118 and #7131 collapses into a few effects:

| Effect | Made through | Covers these reported routes |
|---|---|---|
| Merge a pull request | `gh pr merge`, REST `PUT .../merge`, GraphQL `mergePullRequest`, `gh api` | brace expansion, `$(echo gh pr) merge`, `xargs gh`, script written then run, `find -exec`, `bash <(...)`, `op run --`, alias, `python3 os.system`, `perl exec`, `noglob`, `echo ... \| $SHELL`, plain `curl -X PUT` |
| Push or update the default or a protected branch | `git push origin main`, REST `PATCH git/refs/heads/main`, GraphQL `updateRef`, `{branch}` placeholder of `gh api` | `git push origin main`, GraphQL `updateRef` |
| Create or delete a protected-pattern branch | `git push origin :main`, `POST git/refs`, GraphQL `createRef`, `deleteRef` | GraphQL `createRef` |
| Rename the default or a protected branch | `POST .../branches/main/rename`, `gh api` | renaming the default branch |
| Change the default branch | `PATCH /repos/{o}/{r}` with `default_branch` | `PATCH ... default_branch` |
| Create, move or delete a tag | `git push origin v1`, `POST git/refs refs/tags/..`, `POST git/tags`, `git push --delete` | tags |
| Create a release | `gh release create`, `POST /releases` with a new tag and with an existing one | releases |
| Merge later | enable auto-merge (GraphQL `enablePullRequestAutoMerge`), merge queue | the **[verify]** auto-merge route |
| Run as the repo's workflow | push a branch with a workflow whose `GITHUB_TOKEN` pushes to main | the **[verify]** workflow route (the App has no Workflows permission, so the push of the workflow file is itself refused) |
| Allowed, to prove the chat still works | push a feature branch, open a pull request, comment, edit its own branch | no regression |

GitLab runs the same rows with a Developer-role project token (`glab`, REST, GraphQL, git).

Each verify item of sections 2 and 3 is a row of this table with the forge's answer recorded in
`docs/research/` beside this file once seen. The run also asserts the **bypass actor is the
person's role alone**, by reading the ruleset back.

**Layer 2, a fake forge in the app's tests.** A local server that records each request and answers
what the live run recorded: its refusals as fixtures. It proves: the mint sends the narrowed
`permissions`; the lock check classifies each recorded ruleset answer correctly (locked, a named
gap such as "the chat's App is on the bypass list" or "tags are not covered"); an unlocked repo
refuses a mint; the person's press, and nothing a chat sends, takes the release or tag path; and
the text guard's message names the forge. Per the repo's rule, a test is trusted once seen to fail:
each assertion is first run against a ruleset missing that rule.

**Where it runs.** Layer 2 in CI with the rest. Layer 1 is manual and nightly, with its test-repo
credentials in a vault, never in the repo or the PR, and never part of the merge gate.

## 9. Open questions for the person

1. **The merge of a pull request under *Restrict updates*.** The page read does not say whether a
   non-bypass actor can merge a pull request into a branch whose updates are restricted. If it can,
   do you want the gate App (slice 8), or a required review from a bot only the app has?
2. **Auto-merge.** Should the lock check refuse a repo with *Allow auto-merge* on?
3. **Can a chat reach your own `gh` or git login?** The lock only holds if a chat never holds a
   credential in the bypass list. A chat with no minted token falls back to the machine's `gh`
   login (`crates/purlis-core/docs/forges.md`: "a chat always uses `gh`"). Does the sandbox deny
   that login's files and keyring item today for every harness? This needs its own check, and
   slice 5 should refuse a chat with no token rather than let it fall back.
4. **Plan.** Are the repos on Pro, Team or Enterprise Cloud (private repos need one for
   rulesets)? A repo on a plan without them cannot be locked by this design.
5. **Which branches are protected beyond the default?** Release branches, `main` of a vendored
   fork, anything the person merges as a release? One pattern list per project.
6. **Bypass by role or by user.** The repository admin role is simplest. Should it be your user
   only, so another admin's login is not a bypass either?
7. **Chat pushes.** Chats push their own branches themselves ("you can push yourself", ruling of
   2026-11-10). Keep, or move pushes to the app's `purlis worktree push` and give chats
   `contents:read` plus the pull request permission? The latter needs no ruleset for the default
   branch at all but breaks every harness that pushes with `git`.
8. **GitHub Enterprise Server** is not minted for yet (`apptoken.rs`); out of scope here?
9. **GitLab without Premium.** On GitLab.com free, project access tokens are unavailable, so the
   chat's credential is a dedicated Developer user's token. Acceptable?
10. **Org versus repo rulesets.** One org ruleset covers every repo and cannot be loosened by a
    repo admin, but needs org admin. Which do you want as the default?
11. **A new ADR?** This changes what ADR 0077 (agent identity) says a chat's token is for. An
    amendment file `docs/adr/0077/2026-11-18-<slug>.md`, or a new ADR?

## 10. Sources read

GitHub:
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

GitLab:
- <https://docs.gitlab.com/user/project/repository/branches/protected/>
- <https://docs.gitlab.com/user/project/protected_tags/>
- <https://docs.gitlab.com/user/permissions/>
- <https://docs.gitlab.com/user/project/releases/>
- <https://docs.gitlab.com/user/project/repository/branches/default/>
- <https://docs.gitlab.com/user/project/settings/project_access_tokens/>

Not read, so not relied on: GitHub's pages on auto-merge, merge queue, branch protection rules
and immutable releases. They belong in the verify rows.
