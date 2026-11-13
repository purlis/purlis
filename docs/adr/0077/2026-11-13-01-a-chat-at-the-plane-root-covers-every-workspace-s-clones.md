# Amended 2026-11-13: a chat at the plane root covers every workspace's clones (#6494)

Ruled 2026-11-13 by the person, on #6494 (option A of three). This amends E3 of the
[2026-10-29 amendment](2026-10-29-01-an-agents-github-token-is-minted-from-the-orgs-app.md) for a
chat that works in no workspace. Nothing else in E1 to E5 changes.

## What was wrong

E3 defined the chat's repos as "the github.com remotes of the clones the chat's workspace had
when the chat started". A chat started at the plane root has no workspace, so every `gh` run in
it was refused ("this chat works in no workspace, so it has no repos a GitHub App's token could
be minted for"). The steward's skills run there, and sentry-triage files and labels issues with
`gh`, so it could not run at all.

## E3, amended: the repos of a chat at the plane root

**A chat whose folder is the plane's root has as its repos the union of every workspace's clones
as they were when the chat started.** Every check E3 makes is kept, clone by clone:

- the clone was cloned by purlis or saved from the window, and the remote counted is the one
  recorded for it then;
- its git directory is in no folder a chat may be granted;
- it was there, with the same `.git`, when the chat started. A chat's start already records
  every workspace's clones, so a clone added since is not covered.

A token is still minted for one installation, with `repositories` limited to those remotes, and
at most 500 of them. **Where the repos are on several installations**, the installation is the
one the run's clone is on. A run at the plane root itself is in no clone, so it is refused, with
a sentence for a root chat: "this chat works at the plane root, so its repos are every
workspace's, and they are on several installations of the GitHub App: run the command in a clone
of one of them".

A folder of the plane that is neither its root nor in a workspace is still refused, as before.

## Options not taken

- **Only the repo the command names** (`-R`, `GH_REPO`, `repos/<owner>/<repo>`), checked
  against the same set. That token would be narrower, but the mint would have to parse `gh`'s
  arguments, which is fragile and incomplete (`gh api graphql`, `gh search`).
- **Keep refusing**, and move every skill that needs `gh` into a workspace.
