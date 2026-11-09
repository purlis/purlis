# An artifact's view and approval are keyed by workspace and folder

**2026-11-10, spec #5948, operator's ruling.** Amends §2 and §4 of ADR 0122 so the record says
what the code does.

- **An artifact's view key is `<workspace>/<folder>`**, not `<folder>`:
  `ViewRef { from: null, view: "artifact", key: "<workspace>/<folder>" }`
  (`app/src/artifacts.ts`). A view's key is unique per project, and two workspaces may each pin
  a folder of one name, so a key of the folder alone would make two artifacts one tab. A
  workspace's name holds no `/` (`purlis_core::contain::workspace_name_ok`), so the first `/`
  parts the two.
- **An approval is keyed by the workspace and the folder** as well: by where the workspace
  really is on this machine (its canonical path) and the folder
  (`purlis_core::artifact::approval::key`). §4's "a fork of the workspace asks again" follows
  from it, and so does a plane cloned to another place: each is another key. One approval is
  kept per key, so approving a new version of a folder replaces the old.
