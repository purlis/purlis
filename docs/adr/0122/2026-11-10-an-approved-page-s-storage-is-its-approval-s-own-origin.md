# An approved page's storage is kept in an origin of its approval's own

**2026-11-10, spec #5948, from the second adversarial review of #5978 (findings 1 and 5).**
Amends §3 and §4 of ADR 0122, and the amendment of 2026-11-10 that keys an artifact's view and
approval by workspace and folder.

An artifact's webview stopped being incognito, because WebKitGTK serves an incognito webview
nothing, so what its page keeps (local storage, IndexedDB, a cookie, the Cache API) lasts. It was
kept apart only by the page's origin, and that origin was one per workspace path and folder,
whatever was approved there. So the storage outlived the approval: version 1 of a template,
approved, kept what its queries answered; version 2, approved for no queries at all, read it back
and sent it out through clicks on the links it writes. An unpin and a pin again, or a workspace
made again at the same path, found it too.

- **An approved page is served from an origin drawn from its approval**
  (`purlis_core::artifact::approval::page_host`): 128 random bits drawn afresh with every
  approval and kept in its entry of `artifact-approvals.json` (`storage`), with the pin's id
  and the template's fingerprint. A changed template, a changed pin, or an approval forgotten
  and given again is a new, empty origin. The same approval keeps its origin on every load, and
  across a rename of its workspace, which carries its approval. An approval recorded before
  this is served from its pin and fingerprint alone, so not from the origin it used before.
- **A page whose script does not run here keeps the folder's own origin**, since it can keep
  nothing. History's view of a version (`as-of`) is drawn the same way, as an origin of its own.
- **Unpinning on this machine forgets its approval** (`purlis_core::artifact::unpin_here`, from
  the window and from `purlis artifact unpin`), so pinning it again, even with the same command
  and so the same id, asks again and is a new origin. A teammate's unpin and pin again that
  leaves the pins file as it was is the same pin and the same approved bytes, and keeps its
  origin.
- **A reload that finds the origin moved loads the page there** (`artifact_reload` navigates
  rather than reloads), and where the webview may go is asked of what it is served now
  (`Artifacts::may_navigate`), so the old origin is served nothing. Link approvals, the theme
  sheet and the port are all answered by the label's current origin, so they follow it.
- **What an old origin kept stays on disk, unreachable.** Clearing one origin's website data
  needs an API Tauri does not expose (only clearing the whole store, which on macOS is the
  window's too), or `unsafe` on macOS. A random part in each approved origin means no later page
  is ever served from it again. A per-artifact data store was not chosen: on Linux it is a web
  context of its own, the kind the artifact scheme was found not to reach when the webview was
  incognito, and on macOS removing one needs `unsafe`.
- **Tested in the app on both platforms** (`app/e2e/specs/artifact-isolation.e2e.ts`, case 6):
  a page keeps something every way it can and finds it again on its next load; another
  artifact and the window find none of it; and nor does the same folder's page once its
  template changed, or once it was unpinned and pinned again.
