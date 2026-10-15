# Amended 2026-10-09: the `purlis` binary finds the bundle it shipped in (purlis#1366)

The command line, the footer's badges, the events a command tells and the session-start
briefing now reach the app's built-in extensions. Each passed no bundle before (#340, #342,
#343), because the binary could not tell the app's bundle from a folder claiming to be one.

**Where the bundle is comes from the binary's own real path, and from nothing else.** The
binary resolves its executable through every link, then reads the layout the app ships:

- macOS: `<name>.app/Contents/MacOS/purlis` has its built-ins in
  `<name>.app/Contents/Resources/extensions`.
- Linux: `<usr>/bin/purlis` has them in `<usr>/lib/purlis/extensions`. `<usr>` is `/usr` in
  a `.deb` and the image's own `usr` in a mounted AppImage. `purlis` is the app's product name,
  which is also where Tauri puts the app's resources. The 2026-09-25 amendment's
  `/usr/lib/charter` predates the rename.

No environment variable, setting, record or plane takes part, so nothing a chat can set moves
it. That is stricter than the app's own answer, which on Linux falls back to `APPDIR`. A link
anywhere between the executable's directory and the extensions directory refuses the bundle.
A binary that is not inside one, such as a development build, a test or a copy, reaches no
built-in. Its line after an unknown word says it is outside the app's bundle.

**This is the containment the app already has, not a new one.** A person who copies the binary
next to a folder shaped like a bundle chooses its built-ins. They have also chosen which program
runs, which is the class the 2026-09-25 amendment already accepts for the app. Everything else
built-in trust skips or keeps is unchanged: the executor re-reads at every press, the manifest is
checked, and the operator's "off" holds.

A persona's grant now reads a built-in command's own `"writes": false` too, so such a command
can be waved through. Outside a bundle, a built-in command is not reached at all.
