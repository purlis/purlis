# Amended for #2835, the browser engine sandboxes under a profile or the machine's Chrome, 2026-10-21

The pinned engine purlis downloads makes its sandbox with user namespaces. Ubuntu 23.10 and later
allow them only to a program an AppArmor profile names, so on Ubuntu 24.04 every browser ended at
its start. **Ruled by the person, 2026-10-20: a profile, and the machine's own Chrome, with the
engine's sandbox always on.**

1. **The `.deb` installs an AppArmor profile** scoped to the pinned engine's path in the default
   data folder alone (`crates/purlis-core/src/browsers/purlis-engine.apparmor`, at
   `/etc/apparmor.d/purlis-engine`), as Chrome's own package does. It allows that binary
   `userns` and nothing more. *Ruled.*
2. **Where no profile names the engine**, because purlis was not installed by the `.deb` or the
   data folder is elsewhere, and a browser is refused for it, a Notice offers two ways out: the
   exact command that adds a profile for this machine's engine (`purlis-engine-here`), which the
   person runs since purlis never runs anything as root, and this machine's own Google Chrome or
   Chromium where one is installed. Choosing it is this record's engine choice, kept in
   `machine.json` as `browsers.engine = "machine"`. *Ruled.*
3. **Where nothing works**, the chat is told in words that a browser is not available on this
   machine, never the engine's crash. *Ruled.*
4. **Detected before the engine starts** where it can be: `kernel.apparmor_restrict_unprivileged_userns`
   is `1` and no purlis profile's attachment names the engine's path. Otherwise from the engine's
   log, whose "No usable sandbox" line is said as the same refusal. *Ruled.*
5. **CI installs what the `.deb` installs**: the same profile file, as its postinst does, and a
   tunable that tells AppArmor the runner's test homes are under the temporary folder. *Ruled
   (keep the step only as what the product installs).*

Decisions taken in implementation, to be confirmed:

- **D-2835-1** The host refuses a start with one sentence (`sandboxing::NO_ENGINE_SANDBOX`), and
  the window tells it apart by that sentence, so the session protocol does not change. A refusal
  code would need a protocol minor.
- **D-2835-2** The profile a person's command writes names the engine by its exact path, with any
  version, and is a file of its own (`purlis-engine-here`), so an upgrade of the `.deb` never
  overwrites it.
- **D-2835-3** A chosen Chrome that is no longer installed leaves the pinned engine as it stands,
  with its sandbox check, rather than refusing every browser.
- **D-2835-4** The Notice stands once a browser was refused in this launch, never before, so a
  machine that never opens a browser is not told anything.
- **D-2835-5** The postinst installs the profile only where the setting exists (a kernel that
  restricts user namespaces, so AppArmor 4), since an older AppArmor does not read `userns`.
- **D-2835-6** No chat writes the engines folder (`<data>/engines/chrome-for-testing`) of the data
  home in use, nor of the default data folder under either name: the profile lets a program at
  that path make user namespaces, so a chat could otherwise put one there.
- **D-2835-7** The command's profile writes every character AppArmor reads as a pattern, a
  variable or a quote in the data path as itself, and no command is offered for a path with a
  control character in it.
