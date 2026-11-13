# Amended 2026-11-11: A delete, a vault use and a push that names no branch are shown

Decision 8 grows by two classes and a push it could not class (#6178), and the forge's own
branch protection stays unread. This extends the as-built amendment of 2026-11-10 ("Risk is
shown, as built"); everything it says still holds unless said here.

- **A delete** (`askrisk::Class::Delete`), only where the ask's own command or tool plainly says
  so, never guessed at: `rm`, `rmdir`, `unlink`, `git rm` (but `--cached`), `find … -delete`,
  `git branch -d`, `-D` or `--delete`, `git push --delete` or a refspec with no source
  (`:feature`), each read through `git -C`, `env`, `cd … &&` and `sh -c` as a push is; and an
  edit a harness's structured protocol names a delete (ACP's `delete` kind). Its row says what
  goes ("Deletes build", "Deletes feature/x on origin", "Deletes a.rs and 2 more"). A delete of
  a protected branch on the remote stays a protected push. #1046's wider reading on a harness's
  ask (`docker rm`, `kubectl delete`, a cloud CLI's `delete-*`) is not taken over: what a program
  of its own means by a word is a guess this decision does not make.
- **A vault use** (`askrisk::Class::VaultUse`): an ask whose Allow hands a stored credential
  from a purlis vault on, by its kind, never by its words: a secret fill (into a field of a
  page), a browser login (filled on its site), a pool login (on the pool's runner), a pool tool's
  run (its values sent to the runner) and a run grant that allows a program (its values handed to
  the chat's run). Its row says what goes from which vault to where ("Hands dash-api-key from
  vault qa to a field on dash.example.test"). A Secret ask and a run grant's Fill store a value
  and hand none on; a refused vault's Allow, and a run grant's own vault Allow, tag a vault for a persona and
  hand none on; neither is classed. A run grant that is a vault use is no longer offered a rule
  (#6010); its own Always allow is still there. This narrows the same day's
  [Always allow is offered as a rule](2026-11-11-always-allow-is-offered-as-a-rule-and-harness-rules-are-not.md)
  (#6177): every run grant that allows a reviewed program hands it the vault's values, so the
  rule it offered after the same Allow twice is not offered, by its own last term ("never on an
  ask marked risky"). An ask that carries no risk is still offered its rule, as that amendment
  says.
- **A deliberate press wherever an ask is answered on its row**: an ask answered in the form
  its Answer opens (a secret fill, a login, a pool's asks, a run grant with a value to type)
  asks once more under its answers as a decision does, held by the settle guard; Cancel, or a
  second press of the same grant, takes it back.
- **A push that names no branch** (D-2): `git push`, `git push origin` and a `HEAD` refspec are
  classed by the branch the chat's folder would push, read with git there
  (`askrisk::pushes_from`): its current branch, or, where `push.default` is `upstream`, the
  branch it tracks; "Pushes this chat's branch to main, a protected branch". A line that moves
  first (`cd`, `pushd`, `env -C`, `--git-dir`, `--work-tree`) is not classed by it, since the
  folder is then not the chat's; `git -C dir` is read in that folder. `push.default=matching`
  and a remote's own `push` refspecs are not read: such a push is classed by its current branch
  alone. git is run only for an ask that is a push naming no branch, while the asks are listed,
  off the window's thread.
- **The forge's branch protection is not read** (D-1, decided). The repo's default branch in
  `inventory/repos.json` and the project's `[risk] protected_branches` stay its stand-ins.
  Reading it would mean a call to the forge for every listing of the asks (it changes on the
  forge, not here), a token with admin read on every repo, and a store for its answers with a
  tier of its own (ADR 0069); and an answer that fails or is stale would mark a protected push as
  safe, which the stand-ins never do. A project that protects more names it in `[risk]`.
- `[risk]` gains no key, and nothing new is stored, so no tier is named (ADR 0069).
