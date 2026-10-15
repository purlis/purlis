# Built: publish a project that lives only on this machine (#1820, 2026-10-12)

"Publish to GitHub or GitLab" (#1810, Q9) is the creation's forge half run on a project that is
already there: the same owner lookup, the same `create_repo` and `delete_created_repo`, the same
kept repo and Retry, as the person in the window and through purlis's own credential helper. It
asks for no scope beyond the creation's. Settled while building it:

- **"Lives only on this machine" is read from git**: the project's folder is no repository yet,
  or the top of one that names no remote. A folder inside another repository is that
  repository's, and is not offered.
- **What the take-back covers on this machine**: the `origin` the publish named, and the
  repository it started when the folder was none. A commit of the person's own work in a
  repository that was already there is kept: it is their files, as a save commits them.
- **The project's history is scanned before it is pushed**, as a save's push is (SQ-7): a
  credential-shaped value anywhere in it stops the publish, which names it masked.
- **The repo becomes the project's remote in the push record** (`push-remotes.json`), so its
  automatic saves push there with purlis's helper and nowhere else. A creation records its repo
  the same way.
