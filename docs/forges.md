# Your forge sign-in, and what a chat can do with it

You sign in to GitHub or GitLab once per host, in purlis's window. After that a chat in your repos
runs `gh`, `glab` and `git fetch`, `pull` and `push` with nothing typed and no token pasted. The
chat never holds your token: purlis keeps it in your system keychain and acts for the chat, in the
repos you confirmed, on a short list of things. The decision and its reasons are
[ADR 0128](adr/0128-your-sign-in-brokered-one-approach-for-every-forge.md); this page says what it
means for you.

## Sign in once

Open **Settings › You › Accounts** and press **Sign in** beside the host.

- **github.com and gitlab.com** use purlis's own application. GitHub shows a code and a button that
  opens GitHub's page; GitLab opens its own page. purlis asks GitHub for `repo` and `read:org`, and
  GitLab for `api`. It never asks GitHub for `workflow` at sign-in (see *What waits for your
  press*).
- **GitHub Enterprise Server and a self-hosted GitLab** use an application an **owner** of your
  organization or group made on that host. Press **Use this application**, give its client id and
  the organization or group you expect to own it, and purlis keeps both on this machine, never
  synced. Before the forge's page opens, the window tells you what to check on it: that the
  application is owned by the organization or group you named. Sign in only if it is. A client id a
  teammate sends does nothing until you press. If nobody has made one, the window gives you the text to send an
  owner. Where nobody will, use a token connection (below).
- **More than one account on a host** (a personal one and an enterprise-managed one) is fine. You
  choose which account serves each repo, once, when you clone it or trust it. If that account is
  signed out, the repo says so. purlis never serves it with your other account instead.

Signing in, pinning an application, choosing an account and trusting a clone happen only on your
press, in the window. A chat cannot do them, and neither can the command line.

## Which repos a chat can reach

A chat reaches the repos **you cloned from the window**, or that you told purlis to trust with
**Trust this clone**. purlis records the repo's host, owner and name, and the **number the forge
gives it**. A repo renamed away and replaced under its old name by another one has another number,
so it is not served. A clone made from your own terminal is outside the list until you trust it.

## What a chat can do

In those repos, as you:

- `git fetch`, `pull` and `clone`, and `git push` of **branches the chat made** (a force push with
  lease to its own branch included). A submodule is another repo: it is served only if you trust it.
- Open pull and merge requests, comment, review with a comment or a request for changes, read
  issues, labels, milestones and CI runs, jobs, logs and artifacts, and download release assets.
- Read through `gh` and `glab`, including `gh pr status` and a filtered `gh pr list` for the repo.
- Download the files an LFS repo holds. Dependencies from public repos fetch without a sign-in.

It does that through what purlis sets up for each chat: `gh` talks to a socket purlis opened, `git`
and `glab` through the chat's own proxy with a certificate purlis made for that chat. You do not set
any of it. Run `gh` or `glab` with its own login in your terminal, outside a chat, as before.

## What is refused, and why

purlis asks the forge as you, so the forge cannot tell a chat from you. That is why the refusals
are purlis's, and why they err toward no:

- **Merges, auto-merge and approvals.** A merge is an Ask (below). A chat never approves its own
  work as you: a review with `APPROVE` and GitLab's `approve` and `unapprove` are refused. So are
  merge commands in text (`/merge`, `/approve`, `@mergify`, a bot's `merge`), every GitLab quick
  action, a label that triggers a merge bot, and a label named by its id, which purlis cannot read.
- **Admin.** Repository, branch-protection, ruleset, hook, secret, deploy-key and settings
  changes, anything under `/user` or `/orgs` that writes, and a push to the default branch or a
  protected one.
- **Other repos.** Anything the chat names that is not one of its repos: another repo's issues,
  a search across repos, your other pull requests, a user's profile. A related repo that a
  pull request points at shows its name and nothing else.
- **Sign-in pages.** A chat cannot send you to the forge's device-code page.
- **Direct routes to the forge.** The chat's network refuses the forge's own hosts and their
  addresses, so the only way to a forge is the one above. Download-only hosts for release and
  archive downloads stay open.
- **A forge your administrator has pinned as never allowed.** If your organization's policy pins
  a forge host (github.com, a GitHub Enterprise Server host or a GitLab host) as never allowed for
  chats, chats are refused that forge entirely: `gh`, `git` and the proxy's route included. The pin
  is checked first, before purlis hands a request to your sign-in, and the refusal names the pin
  and who set it. Your own sign-in does not override it; ask the administrator to lift the pin.
  Your own terminal, which is not a chat, still works.

A refusal tells the chat why and what it can do. If a tool you run fails with a certificate error
on a forge host, it does not know the chat's proxy; use `gh` or `git`.

## What waits for your press

These are not refused. The chat asks, the ask appears in **Asks**, and **you** press. purlis then
makes the call with your sign-in and the facts you were shown.

| Act | What you are shown |
| --- | --- |
| Merge | The pull request, its head commit, the base and its checks. The merge sends the head you saw: a head that moved is refused by the forge. |
| Tag | The repo, the name, the commit and whether it is on the default branch. A tag that exists is never moved or deleted. |
| Release | The same, plus the title and notes. The release is made against a tag at the commit you saw. |
| Fork | The repo and your own account as the only place it goes. A fork to an organization is refused. |
| CI-file push | The branch, the changed lines of the CI files and the commit message. |

Each takes **two presses**. A CI-file push is never approved by default and never remembered. If
nobody is there to press (an unattended runner), the chat is told *Held: nobody is here to approve
it.* and nothing happens until you are.

**What counts as a CI file** is decided by the path, never by reading the file:

- **GitHub:** anything under `.github/`, and any file named `action.yml` or `action.yaml`.
- **GitLab:** any `.yml` or `.yaml` file anywhere, and the project's pipeline file.

On GitLab that means a push that only edits a YAML file nothing includes (a data file, Helm
values, an OpenAPI document) asks too. purlis accepts the extra ask because a pipeline file read
two ways by two parsers is worse, and it asks for every YAML edit rather than guess.

**GitHub's CI-file push needs a second sign-in.** The first time one is asked for, the window
offers **Allow CI files** in Settings › You › Accounts: a second GitHub sign-in that asks for
`workflow`, kept apart from your everyday one and used only for a push you approved. If GitHub
adds that permission to your everyday sign-in instead, purlis shows a Notice, serves that account
to no chat until you sign in again without it, and does not offer the push; push it from your
terminal. GitLab has no second sign-in: its `api` permission covers CI files, so the Ask is the
protection.

## What purlis does not hold

purlis tells you where its protection stops, so you can decide what else a repo needs:

- **Code a workflow runs.** A pushed branch makes the repo's existing workflows run, with its
  secrets, on code the chat wrote: a script a workflow calls, or a local action's code outside
  `.github/`. Only the forge's own settings stop that: environments with required reviewers,
  secrets limited to protected branches. purlis recommends them and never requires them.
- **Other CI systems.** On a GitHub repo, a `Jenkinsfile`, `.circleci/`, `.travis.yml` and the like
  are not CI files to purlis.
- **Reading a commit of another repo by its id.** GitHub shares objects between a repo and its
  forks, and purlis cannot tell which repo holds a commit, so a chat can read a commit it already
  has the id of.
- **A bot or workflow with its own command or label.** purlis refuses the commands and labels it
  knows. A repo's own bot that merges on a word is not one it was told about.
- **A chat you ran with no sandbox.** It gets the same route, and purlis cannot stop it using your
  own `gh` login.
- **Tools outside purlis's route.** A chat with the network off, or a program that ignores the
  proxy, meets the sandbox's own network rule instead.
- **What the forge cannot enforce.** The forge sees you. It cannot refuse "the bot", and its log
  says you did it. purlis's per-chat audit says which chat.

A repo that wants a forge-side backstop for its default branch (a rule that requires a pull
request and a review) gets one that binds a chat as it binds you.

## Check that your organization is ready

One thing can fail without a good message: an organization that has **OAuth app access
restrictions** (on by default) refuses an application no owner approved. Run **`purlis doctor`**
or open **Settings › You › Ready for all users** with a project open. purlis reads the forge as
you, in read-only requests, and says per check *Ready*, *Blocked* (and whose to fix: you, the owner
or the administrator) or *Not read*. A setting it cannot read counts as not ready. The checks
are your role in the repo, your organization's approval of purlis's GitHub application, the
application on a self-hosted GitHub or GitLab server, whether a GitLab server lets users register
applications, the default branch's protection and, where a token is used in CI, whether it may
push.

**If you own the organization:** open the organization's settings on GitHub, find *purlis* under
the third-party application requests, and press **Grant**. Approve it once and it covers everyone
in the organization. A blocker the window shows comes with the clicks and the text to send an owner
when it is not yours to fix.

Two cases are your own step: an organization with **SAML single sign-on** needs an active SAML
session when you authorize, and you authorize purlis again after an owner enforces it. GitLab
groups that enforce **SSO for Git** can refuse git over HTTPS for your sign-in while the API works;
a group owner checks that setting. purlis speaks git over HTTPS only, so an instance must allow
it. purlis cannot detect an enterprise policy (EMU), a collaborator's disabled requests or some IP
allowlists: it shows the guide anyway.

## Renewal, and when you sign in again

You do not sign in because a token ran out. purlis renews a GitLab sign-in before it expires, without you. A GitHub
application's token does not expire, as GitHub documents, so there is nothing to renew. You sign in again only when:

- **you or an owner revoked** purlis's access, or GitHub revoked a token **unused for a year**;
- **a SAML organization** needs you to authorize again;
- **GitLab** says the refresh token is over, or another application is pinned for the host;
- **the account changed**, or the forge flagged it.

Each shows as a Notice with the fix in one press, never as a failed push. Revoke a sign-in on the forge's own page of authorized
applications.

## CI, servers and runners: token connections

Where no one signs in (CI, `purlis serve`, a pool runner), a **token connection** gives purlis the
credential. The proxy and its refusals are the same.

- **Names.** For one host, `PURLIS_FORGE_TOKEN__<HEX>` or `PURLIS_FORGE_TOKEN_FILE__<HEX>` (a path,
  for a mounted secret), where `<HEX>` is the lower-case hex of the host's bytes (`github.com` is
  `6769746875622e636f6d`); the window and `purlis doctor` show the name for a host. purlis also
  reads each forge CLI's own variables (`GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GITLAB_TOKEN`,
  `CI_JOB_TOKEN`), **each offered only to the host it names**. `docs/credential-sources.md` has the
  whole table.
- **Which tokens.** Installation tokens, fine-grained tokens and GitLab's tokens are taken. A
  classic `ghp_` token is refused: it reaches every repo its owner can.
- **Renewal.** None. A token that is given and unusable is the answer, and nothing falls back to
  another. For a long-lived token, give its expiry in
  `PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>` and purlis warns 14 days before.
- **Limits.** `GITHUB_TOKEN` reaches only the repo the workflow runs in and cannot push a workflow
  file. `CI_JOB_TOKEN` reaches a short list of GitLab endpoints, so most `glab` writes through it
  are refused by GitLab, not purlis.
- **Repos.** With nobody to press, the repos are those in `PURLIS_FORGE_REPOS` (a comma-separated
  list of `host/owner/name`) or the checkout's `origin`.
- **Runners.** A runner's chat keeps its token connections until pairing with the desktop's proxy
  lands. A mint no longer hands a runner chat an installation token.

## Moving from Agent identity and vault GitLab tokens

Agent identity (a GitHub App's token for a chat) and a GitLab token kept in a vault are replaced by
the sign-in. With one login on:

- **A tool line** that asks for a GitHub App token (`mint = "github-app"`), hands a vault value in
  `GH_TOKEN`, `GITLAB_TOKEN` or another forge token variable, or names `gh` or `glab` still loads
  and runs nothing. A Notice and a `purlis doctor` row say to sign in and delete the line.
- **Keep a `mint` line while the App's key is in a vault.** Deleting it unmarks the key. Remove the
  key on your press once you are done.
- **A vault item** purlis recorded as a forge token is refused to a chat. You at your own terminal
  are not refused.
- **Agent identity and GitLab access** no longer set anything up. Their removal paths still work.
  The organization's GitHub App stays installed, untouched: an owner may uninstall it.
- **Nothing is deleted for you.** purlis removes its own record and, on your yes, the vault key.
- **A teammate on an older build** keeps the old route until they update. The two builds agree
  about the repo, and differ only about the credential.
- **A host where nobody can register an application** (GitLab) takes the old vault token as a
  token connection that you paste in the window. It lives in your keyring, and the vault item is
  offered for deletion.

## Related

- [Getting started with GitHub](getting-started/github.md) and
  [with GitLab](getting-started/gitlab.md).
- [Forge tools: which commands never run with a vault's value](forge-tool-refusals.md).
- [ADR 0128](adr/0128-your-sign-in-brokered-one-approach-for-every-forge.md) for the decision, its
  residuals and why.
