# Amendment, 2026-11-21: held forge acts (ADR 0128 D8)

A chat holds no forge credential (ADR 0128), so the acts that are hard to undo are not refused dead.
The merge hold of §7 is **generalised**: the chat asks, the person approves in Asks, and the app
performs the act with the person's sign-in. This amendment says what that changes in §7 and in the
two-press rule of 2026-11-14. ADR 0128 D8.5 names the items; each is quoted here with what it now
says.

**§7 item 3** (held pushes: *"A push to a protected branch, or a bare force push, is held"*;
*"`--force-with-lease` to the chat's own branch isn't held"*). Now **also tags, releases, forks and
pushes that touch CI files** are held, each with an Ask that shows exactly what will happen:

- **A tag** shows the repo, the name, the commit and whether that commit is on the default branch.
  A tag that exists is never moved, and a tag is never deleted by a chat: both are refused, not held.
- **A release** shows the same, plus its title and notes, and is bound to the commit shown: the app
  makes the tag at that commit first and the release against the tag, with no `target_commitish`.
- **A push that touches CI files** shows the branch, the new tip, what it was compared with and the
  changed lines of the CI files. The diff is read from a quarantine the app owns, from the base the
  forge holds and the pack the chat pushed, with git hashing every object; it is never read from the
  chat's clone, whose objects a chat can overwrite. On GitHub the push uses the second credential
  (the `workflow` scope), on GitLab the everyday sign-in.
- **A fork** shows the source and the person's own account as the only place it goes.

The lease to the chat's own branch stays allowed.

**§7 item 4** (*"Switching the merge hold off means purlis stops intercepting merges: the forge and
the chat's own token decide"*). There is no chat token, so there is nothing to fall back to. The hold
can be set to "ask every time", never to "let the chat merge".

**§7 item 5** (*"That list, or a brokered forge token (a later design, outside these phases), is a
precondition for offering 'purlis decides'"*). The brokered forge token is the proxy of ADR 0128.
**The "purlis decides" precondition never applies to a held act**: no setting, list or mode lets
purlis approve a merge, a tag, a release, a fork or a CI-file push in the person's place. Where
"purlis decides" is offered at all, it is for acts the allowlist already allows. In the code, no
grant exists for any held act, and a press is taken from the window's own connection alone.

**§7 item 9** (*"A stated gap … a script that merges with the chat's token gets past purlis"*). With
no chat token the gap shrinks to a script that merges through the proxy's route, which the proxy
recognises by its request. A script that calls the forge in another way has no credential to do it
with. The remaining gap is a chat with no sandbox.

**§7.2** (the merge row's content: *"title, author, base and checks are read from the forge"*; the row
names the repo and the head SHA). It adds that the merge call sends the viewed head SHA, so a head that
moved is refused by the forge and not only redrawn on the row.

**The two-press rule (amended 2026-11-14)** stays: a merge takes two presses. **A tag, a release, a
fork and a CI-file push take two presses as well**, because each is as hard to undo as a merge. The
second press is the same "asks once more" step with the facts read again from the forge, and it
names the nonce the first press returned, so a double click of the first press performs nothing and a
double click of the second performs once. A CI-file push is never approved by default and never
remembered, and nothing is performed where nobody is present: the chat is told *"Held: nobody is here
to approve it."* and the request waits for a connected desktop.

**What the chat is told.** A request the proxy recognises as one of these acts is not forwarded. The
chat gets a 403 with one sentence that names the ask: *"purlis holds tags. It has asked the person;
the tag happens when they approve it in Asks. Ask 3: tag v1.4.0 in acme/app."* A yes typed in the chat
is not the press.

## Which pushes are CI-file pushes: by path, never by parsing (round 6 of #7299)

**The rule is on the path alone.** A push is held as a CI-file push if it adds, changes, deletes or
renames (a rename is the removal and the addition it is, so both paths count):

- **on GitLab**, any file whose name ends in `.yml` or `.yaml`, in any case, anywhere in the tree,
  or the project's configured pipeline file (`ci_config_path`);
- **on GitHub**, anything under `.github/` (workflows, local actions, reusable workflows, and what
  they reference), or a file named `action.yml` or `action.yaml`, in any case, anywhere, which a
  `uses: ./path` can name;
- on both, the dependency bots' configuration and `CODEOWNERS`, as before.

`ci_config_path` is read from the forge's project API at prepare time, as the person, and never
cached. A project record that cannot be read, or that does not carry the field, **refuses the ask**
with a sentence that says why; purlis does not guess the default.

**Why parsing was dropped.** The first design worked out which files a pipeline runs by reading its
`include:` graph with a YAML parser (`yaml-rust2`). Five rounds of review each found new inputs on
which that parser and GitLab's own loader read a file differently: merge keys, tags, byte order
marks, line breaks the two spell differently, `---` handling, symbol keys, `spec:inputs`
interpolation of keys, a multi-document split the loader does itself. Each fix narrowed the gap and
none closed it, because the decision depended on two readers agreeing on every file a chat can write.
A GitLab `local:` include must be a `.yml` or `.yaml` file, so the name rule covers every local
include without reading one. `strict_subset`, the tag audit, the alias audit and `includes_of` are
gone, as is the `yaml-rust2` dependency of `purlis-core`, and the ask no longer lists what a
pipeline runs from outside the repository.

**The trade-off.** On a GitLab repository a push that edits only YAML nothing includes (a data
file, a Helm values file, an OpenAPI document) asks for the person's press where it did not need to.
That cost is accepted: an extra ask is visible and cheap, and a pipeline file read differently by
two parsers is not. What the rule cannot name is a file a pipeline or workflow runs that is not YAML
and not under `.github/` (a script a job calls, a local action's `index.js` outside `.github/`):
those are not held by this rule, and are the forge credential's to bound (ADR 0128).
