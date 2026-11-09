# A purlis link names a place of a closed list, and only navigates

**2026-11-09, #5967.** Amends §5 of ADR 0120 with the format of a `purlis://` link in an
artifact's page, and what following one does. The format is in
[`docs/artifact-links.md`](../../artifact-links.md).

- **Four places, one shape each.** `purlis://workspace/<workspace>` focuses the workspace,
  `purlis://todo/<workspace>/<todo>` opens the todo's view, `purlis://chat/<chat id>` shows the
  chat (its ULID, ADR 0066), and `purlis://issue/<tracker key>` shows the chat open here that
  works on the work item (ADR 0088), with the key's `#` written `%23`. A todo's tracker key goes
  to the todo. purlis has no view of a forge issue of its own, so the chat linked to it is the
  issue's place in purlis; a page that wants the issue's own page links to it over `https:`.
- **A link names a place, never an act.** Any other first word (`run`, `dispatch`, `settings`),
  anything after a place (`purlis://todo/alpha/fix-login/done`), a query or a fragment, a name or
  a port, and a name that is not one are refused with a sentence the window shows. No link
  carries a parameter, so none can hide an action in one. The core reads the link
  (`crates/purlis-core/src/artifact/places.rs`).
- **The place must be there.** The app checks it against the artifact's own project as it is
  now: a workspace of the project, an open todo of it, a chat open in this run, a chat open here
  linked to the issue (`app/src-tauri/src/artifact_places.rs`). Otherwise it is refused with a
  sentence, and nothing happens.
- **It is gated as an `https:` link is**: only a person's click, only on a link written in the
  template purlis served (#5956). It asks nothing more, because going somewhere in purlis is what
  the person's own press on that row does.
- **The window only navigates.** It hears one of `workspace`, `todo` or `chat`, and runs the row
  that focuses the workspace, opens the todo's view or shows the chat, as its palette would
  (`app/src/artifactPlaces.ts`); those three verbs are the only ones a link reaches.
