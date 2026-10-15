# Amended 2026-09-25: the plugin is called charter (#406)

The first item above is settled. **The plugin the app bundles is named `charter`**, so its
skills reach the model as `charter:<skill>` and a session loads it as `charter@inline`.

**The Python charter's plugin is named `charter` too**, as `charter@charter`. Only the
marketplace after the `@` tells the two apart, and Claude Code loads one plugin per name
(measured on claude 2.1.282, in `charter_core::plugin`'s header): with both available the
`--plugin-dir` copy is loaded, and a session that turns `charter@inline` off gets
`charter@charter` in its place, skills and hooks included. So the pins carry the separation
the old name used to. Every chat's `--settings` pins `charter@inline` on, which a chat cannot
move and which keeps the Python plugin out. It pins `charter@charter` off, which does not
touch `charter@inline`. Each pin matches the whole id, and tests hold both directions.

**The old id is pinned off, not migrated.** charter never wrote `charter-app@inline` into a
file. It was only ever in a session's `--settings`, so no plane's `.claude/settings.json` holds
it unless somebody put it there. The only files that could name it are a project's
`[harness_plugins.claude]` and a workspace's `settings.harness_plugins.claude`. Those the
settings tab never writes for a pinned plugin, and a person would have put it there by hand.
`charter-app@inline` is now a third pin, always off. A file that says `false` agrees with it.
A file that says `true` is refused by the settings tab's save and ignored at a chat's start,
with a sentence that names `charter@inline`. A skill named `charter-app:<skill>` in a
persona's `skills:` is the persona's own text, and the plane changes it.

**The Tauri crate and binary stay `charter-app`, and the bundle identifier stays
`dev.charter.app`.** Neither reaches the model or a plane's files, and renaming the crate is
build churn with nothing to show for it.
