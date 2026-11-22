### Fixed

- **A chat may merge through its forge route where you set `guards.merge` to `log` or `off`.**
  The merge guard already let a merge through under those settings, but the route a chat's `gh`,
  `glab` and git use to reach its forge still refused every merge. Now `gh pr merge` (with
  `--auto` and the merge queue), `glab mr merge`, GitLab's merge when the pipeline succeeds and a
  merge by the API go through for a repo your chat's own run recorded. `log` writes one line to
  the chat's enforcement log, naming the chat and the repo and never the request; under
  `enforce` a merge is still asked in the window. A tag, a release, a ruleset, the repository's
  administration, another repo and a branch merge stay refused, an administrator's lock still
  wins, and a policy purlis cannot read is `enforce`.
  A merge that names its pull request by id, as `gh pr merge` does, is checked against the repo the
  forge says that pull request is in, so a pull request of another repo is still refused and one the
  forge cannot place is too; `gh pr close` works for the same reason. GitLab's
  `merge_when_pipeline_succeeds` is taken in the URL as well as in the body, once and spelled
  exactly so; a repeated or re-cased one is still refused.
  The route answers what the chat's own hook does, per chat: a chat dispatched by an asker that ran
  under `enforce` is held even where its project says `log`, and a loosening you press while a chat
  is open reaches it with its next start (a tightening, at once).
  Nothing is asked of the forge, with your sign-in, for a caller that has not proved it is the chat.
  **Also changed, in every mode:** a request that names a pull request or issue by id in the repo
  your chat recorded (`gh pr close`, a comment, updating a pull request's branch) now works where
  it was refused before, `enforce` included.
  **Only where you opted in:** the route forwards a merge only when your committed `[policy]`
  itself says so, with `guards.merge = "log"` or `"off"` or `template = "open"`. A project with no
  `[policy]`, or on the default `watch` template without a `guards.merge` key, keeps today's
  behaviour: the route holds every merge for your press, though the merge guard in the hook (which
  runs `watch` as `log`) lets it through.
