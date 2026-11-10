# Amended 2026-11-10: Risk is shown, as built

Decision 8 is built (#6011, taking over #1046's protected push and production host), with these
particulars.

- **Three classes, read in the core** (`purlis_core::askrisk`), each from what the ask already
  carries and what the project already knows, never from what a chat says of itself:
  - **A push to a protected branch**: a `git push` whose refspec names one (`main`,
    `HEAD:main`, `:main`, read through `git -C`, `env`, `sh -c` and the push's own options), or
    that pushes every branch (`--all`, `--mirror`). A branch is protected where it is a repo's
    `default_branch` in `inventory/repos.json`, or matches `[risk] protected_branches` in
    `charter.toml` (a name, or a pattern with `*`). A push that names no branch pushes the chat's
    current one, which the ask does not say, so it is not classed. **The forge's own branch
    protection is not read**: it would need a call to the forge for every ask, and a store for
    its answer; the default branch and the project's list stand in for it.
  - **A host that looks like production**: `prod` or `production` as a label of the host, split
    on dots and dashes (`api.prod.example.com`, `prod-db.internal`, never `product.example.com`),
    or a host `[risk] production_hosts` names, or a subdomain of one. Read from a sandbox's host,
    and from every URL in a command line or a tool's input.
  - **A write outside the project**: a sandbox's folder, or an edit's file, not under the
    project's root, on the path as written (`~` is the home folder, `..` is taken back; links on
    disk are not followed). A relative path is the chat's own folder's, which is in the project.
- **An ask carries its risk** (`Shown.risk`: its class, its words, and what its Allow asks once
  more), filled where the registry lists the asks (`asking::derive`), from the project's facts
  read once per listing.
- **On the row**: "High risk" in the waiting tone, then its words ("Pushes to main, a protected
  branch"), first in its body and part of its description, so a screen reader hears it with the
  row (#6020). The row is marked `data-risk`, which keyboard triage passes by (#6016).
- **A deliberate press**: a grant on its row asks once more under the line ("Allow a push to
  main?"), with that Allow and Cancel; only the Allow there answers, and the settle guard holds it
  as it holds every grant. A refusal acts on its press.
- **One predicate** says an ask is high-risk, `highRisk` in `app/src/asksSame.ts`: it is never
  grouped (#6009), never offered as a rule (#6010) and never landed on by triage (#6016).
- The delete and secret-use classes #1046 put on a harness's ask (`harness::model::Risk`) are
  not shown here: decision 8 names these three.
- `[risk]` is a table of `charter.toml`, which is already Plane tier; nothing new is stored, so
  no tier is named (ADR 0069).
