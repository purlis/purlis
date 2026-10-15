# Amended 2026-09-25: a built-in extension is trusted through the app

purlis#339, the first change in [ADR 0053](../0053-an-extension-is-granted-capabilities-one-at-a-time.md)'s
build order. Persona statistics now ships inside the app, in the bundle's resources
(`Contents/Resources/extensions/` on macOS, `/usr/lib/charter/extensions/` in a `.deb`, and
`$APPDIR/usr/lib/charter/extensions/` inside an AppImage). It is a **built-in extension**: the registry lists it with `source: app` and treats it as
approved with no prompt. This is a trust decision, so here is its threat model, by class.

**What makes an extension built in is where it is, and only that.** The app finds its resource
directory when it starts and hands it to the core (`extension::BuiltIn`). An extension is built in
when its directory is directly inside that one, and it is approved at that path and no other.
Nothing a file says can make an extension built in:

- **A write to the record.** A row with `"source": "app"` holds one thing, whether the operator
  turned that built-in off on this machine. purlis reads nothing else from it, not a path and not a
  fingerprint. A record row that claims a directory is the app's grants that directory nothing.
- **A copy of a built-in.** The same directory with the same bytes, anywhere outside the running
  app's resources, is an ordinary extension and reads as new. purlis refuses to install an
  extension whose id is a built-in's while the app ships that built-in. One id is one extension, so a
  copy takes its own id. A row the operator installed before the app shipped the extension under
  that id is set aside and said: that is the hand-assembled persona statistics on the day this
  lands.
- **A link.** A built-in's directory has to be a directory, not a link out of the bundle.

**Where the running app is, is Tauri's answer (`resource_dir`), and it is not a boundary either.**
On Linux it is the directory beside the executable (`../lib/charter`), and Tauri falls back to the
`APPDIR` variable for an AppImage. So a person who copies the executable next to a
`lib/charter/extensions` of their own, or who starts the app with `APPDIR` set, chooses its
built-ins. They have also chosen which program runs, or its environment, and either of those
already runs code as the app (`LD_PRELOAD` needs no extension). The same class as the next
paragraph.

**A writer into the app bundle gains nothing it did not have.** Whoever can change
`Contents/Resources` can change `Contents/MacOS/charter-app` too, and replacing the app is strictly
more than replacing one of its extensions. So purlis does not fingerprint a built-in. Any bytes
inside the bundle are trusted, which is also how an update's new version of the extension is
trusted without asking. What covers those bytes is what already covers the app:

- On macOS, the bundle's seal. The extension's program is a resource, and the seal hashes every
  resource, which `codesign --verify --deep --strict` checks in the release workflow. It is sealed
  as a resource, not signed as nested code. Notarization is declined (ADR 0042), so nothing asks for
  more today. A later decision to notarize has to move the program to where nested code goes.
- On Linux, `/usr/lib/charter` is root's in a `.deb`, and an AppImage is one file.
- For an update, the updater's minisign signature is over the `.app.tar.gz` and the AppImage,
  which contain the extension (ADR 0042). A built-in's new bytes arrive only through a verified
  update.

**What built-in trust skips, and what it does not.** It skips the approval prompt and the
fingerprint comparison, and nothing else. The executor still re-reads the directory at every press
and refuses what it cannot read. The manifest is still parsed against this purlis's vocabulary,
and an unknown capability still refuses it. The program still runs one question at a time, with the
same empty environment, deadline and bounds, and is handed only what `handed.rs` hands any
extension. A project or a workspace still turns it off as it turns off any extension (ADR 0048).
Off on this machine, it contributes nothing and the executor starts nothing. It is never removed,
because the app would bring it back at the next read. Windows still refuses (ADR 0031). A record
purlis cannot read leaves every built-in off, since it cannot say whether the operator turned one
off. That record cannot make an installed extension approved either.

**Persona statistics links the core, and that is a first-party decision, not a change to the
contract.** It links `charter_core::personaverbs::stats`, the code `purlis persona stats` counts
with, so the view and the CLI give the same numbers. It reads nothing more than before: purlis
still hands it names, the default, and one date per memory, and linking the core gives it no reach
that `RUNS_AS_YOU` did not already say it had. The date is now the day `memstore.memory_date` finds,
the stamp line's date or a `YYYYMMDD-` file name, where it was the stamp's minute. That hands
less, and it dates a memory as the CLI does. It is the same protocol, so nothing re-asks. The claim
this extension used to carry, that a stranger can meet the contract without linking the core, is now
carried by `extension-probe`, which links nothing of purlis's.
