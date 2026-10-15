# Built: create a project on a forge, taken back on Cancel (#1819, 2026-10-12)

"Create a project" on GitHub or GitLab (#1810, Q8) asks the forge through the native client as
the person in the window, on their signed-in account, and pushes the first commit through
purlis's own credential helper (ADR 0070, amended 2026-10-12). It asks for no scope beyond §2
as amended: `repo` and `read:org` on GitHub, `api` on GitLab. Settled while building it:

- **The repo is made as late as it can be.** The project is laid out and committed on this
  machine first; only then is the repo made, and the push is the one stage after it. A failure
  or a Cancel before that leaves nothing on the forge.
- **Only the repo this run made is ever deleted.** The forge seam's `delete_created_repo` takes
  nothing but `create_repo`'s own answer: no name, path or id typed by anyone. On GitHub, whose
  delete names a path, purlis first reads the repo at that path and deletes it only while its id
  is the one the create answered. On GitLab, the delete names that id.
- **GitHub's delete needs `delete_repo`, which purlis does not ask for.** GitHub refuses
  `DELETE /repos/{owner}/{repo}` to an OAuth or classic token without the `delete_repo` scope,
  whatever else it holds. With §2's `repo` alone, a push that fails or is cancelled after the
  repo was made leaves that empty repo, and purlis says so: the repo's address and its settings
  page, where the person deletes it. A fine-grained token with Administration: write, and a
  GitLab token with `api` on a project its account owns, are taken back in full.
- **Decided (dispatcher, 2026-10-12): `delete_repo` is not asked for.** Least privilege comes
  first: the scope would let purlis delete any repo the person can, which is far more than
  taking back its own. So on GitHub the take-back stays partial. The repo a failed or
  cancelled push leaves is kept, empty, and named, with its settings page for deleting it by
  hand. The obvious next step is Retry, which pushes into that same repo instead of making
  another: the app keeps the core's own answer for the same forge, host, owner and name, and the
  window never names a repo.
