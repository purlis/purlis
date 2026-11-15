# Amended for #655, the host as a login item and Quit to the background, 2026-11-15

§2 gives X24's ruling: the session host is a login item only when the person has a trigger or
a paired remote device, shows in the tray with a one-click stop, and otherwise ends with the app.
§3 gives its supervision: a launchd agent or a systemd user unit once it is a login item, and the
window only where no manager may be used. #2007 (D-2007-1) left the units to this ticket. FD-17
builds both. §2, §3 and the amendments above stay as accepted, and this note adds to them. Each
item was **decided in implementation, to be confirmed** (D-655-1 to D-655-9).

1. **One place answers "a trigger or a device"** (`purlis_core::loginitem::reasons_here`). It is
   read at each decision and never cached: as the window starts (who supervises the host), at
   each quit (to the background or not) and at the exit (whether the login items stay).
   Neither exists yet. Local triggers are AC-9 (M38, #560), and a paired device is the relay's
   phone or PWA (ADR 0078's amendment of §2). So the answer is none, and nothing changes for
   anyone until one of them lands and adds itself there. A development build reads
   `PURLIS_KEPT_RUNNING_FOR` as a stand-in trigger, for the check on hardware. A release build
   never reads it. *Rejected:* a setting the person switches on ("keep purlis running"). X24
   ties the login item to what needs it, and a setting with nothing behind it would keep a host
   running for no work. *D-655-1.*
2. **Quit goes to the background while something keeps the host running.** Nothing ends. Every
   window is hidden, the Dock icon goes (macOS's accessory policy), and the tray reads
   *Running in the background for your trigger "…"*, then **Open purlis** and **Stop purlis and
   end every chat**. Stop is a quit as it always was: the window's own quit path ends every
   chat and tells the host `quit`, so the host stops. Its login items stay while the trigger
   does, so it starts again at the next login. Quit asks nothing on the way, since it ends
   nothing. The same holds when every window goes (`ExitRequested` with no code). An exit with
   a code always goes ahead: Stop, restart to update (Tauri's `RESTART_EXIT_CODE`) and the
   restart onto the session bus. A macOS logout or shutdown ends the event loop without an
   `ExitRequested` (`applicationWillTerminate`), so nothing holds one up.
   Where no tray shows (on Linux, nothing on the session bus owns
   `org.kde.StatusNotifierWatcher`, as on GNOME without an extension), Quit hides nothing: there
   would be no way back and no way to stop. The window stays and says why, with **Stop
   purlis**. Opening purlis from the Dock, the Finder or a notification brings it out of the
   background as **Open purlis** does (`windows::raise`).
   *Rejected:* the host drawing a tray of its own. That puts an AppKit application inside a
   process launchd starts from the app's bundle, which LaunchServices takes for the app, so
   opening purlis would bring forward a process with no window. *Rejected:* letting the app
   exit and the host carry on unseen. X24 asks for a tray presence. *D-655-2.*
3. **The tray after a login is the app's, started in the background.** A second login item
   starts the app once at login with `--in-the-background` (a launchd agent with `RunAtLoad`
   and no `KeepAlive` on macOS, an XDG autostart entry on Linux). The main window is built
   hidden and the app goes to the background. A login item that outlived what kept the host
   shows the window, and the next quit removes it. *D-655-3.*
4. **The host's manager.**
   - On macOS it is a launchd agent, `~/Library/LaunchAgents/dev.purlis.host.plist`, in the
     GUI session (`LimitLoadToSessionType` `Aqua`, so the Keychain answers the app's binary
     without a prompt). It has `RunAtLoad`, and `KeepAlive` on an exit that was not a success.
     It is loaded with `launchctl bootstrap gui/<uid>`, started with `kickstart` and held with
     `bootout`. When this build's agent differs it is written again, but a host launchd runs is
     never booted out for it, since that would end every chat: launchd reads the new agent when
     it next loads it. A bootstrap refused because the agent got loaded meanwhile (launchd's
     error 5) is no failure. Installs run one at a time.
   - On Linux with a user manager it is a systemd user unit, `purlis-host.service`, enabled
     into `graphical-session.target`, with `PartOf=` and `After=` it, so the host and the
     browsers it runs have the session's `DISPLAY` or `WAYLAND_DISPLAY`. It has
     `Restart=on-failure`, `StartLimitBurst=3` and `StartLimitIntervalSec=300`.
   - A development build's labels, unit and autostart entry carry `development`, so it never
     loads, rewrites or removes a release build's.
   - Both run the app's own binary as `purlis-serve-as-a-login-item`.
   - Where no manager may be used (Linux without a systemd user manager, Windows) the window
     supervises as before. The app's background still keeps a host the window runs, but nothing
     starts it at login.

   *Rejected:* `SMAppService`. Its calls are `unsafe` Objective-C, which would be a sixth
   audited block, and it needs the agent's plist inside the signed bundle. A file in
   `~/Library/LaunchAgents` and `launchctl` are launchd's own documented interface, and they
   work on every macOS the app supports. *D-655-4.*
5. **A host that is a login item has no grace period.** A window that goes, with or without a
   word, ends nothing, and no project is let go of for want of a window. It lives until told to
   quit (`Hosting::while_it_is_a_login_item`). A program started to end with its window still
   ends as that window goes. Finding another host serving, it exits 0. A window started one
   first, and launchd and systemd restart only after a failure. *D-655-5.*
6. **The window hands supervision to the manager** (`sessionhost::managed`) when something keeps
   the host running, there is a manager, and the chats run in the host. A window whose chats run
   in its own process supervises its host itself, since a host it did not start does not take a
   peer below it as inside a chat.
   - The window installs the login items, asks the manager to start the host, waits for it to
     answer on its socket, and connects. Each link that ends is the manager's to mend, and the
     window connects again.
   - **One breaker whichever manager.** systemd's tripped limit is read from the unit's
     `Result` (`start-limit-hit`, from `systemctl show --value`, its interface for scripts).
     launchd never gives up, so the window's own breaker counts lost links by the same limits
     and tells launchd to stop (`bootout`).
   - Either way the window raises `session-host-failing`. On macOS it shows the last lines of
     `<app log>/session-host.log`, where launchd writes what the host says. Try again lets go of
     the hold (`reset-failed`, or loading the agent again) and starts the host. systemd's limit
     reads as *your system* stopped starting it, and the window's as purlis.
   - **A manager that cannot run the host hands it back to the window.** When it will not
     install or start it (purlis's background item switched off in System Settings, a failed
     `systemctl`), the window supervises the host as before, at once and never through the
     breaker, and a Notice says, in plain words, that purlis will not start at login and what
     to check.
   - A quit the host never answered (no link, or no answer in time) has the manager hold it,
     so no chat outlives the quit.

   A host the window runs when a trigger first appears goes on as it is until the next launch:
   handing live chats from one host to another is FD-28 (#665). *D-655-6.*
7. **The login items go once nothing keeps the host.** A quit (or Stop) that finds no trigger
   or device tells the host to quit, then removes both login items, and only when they are
   there, so nobody without a trigger or a device has launchd or systemd asked anything. A
   Stop while one remains leaves them. *D-655-7.*
8. **What needs a real login to prove waits on the operator**: that launchd and systemd start
   the host at login and the app in the tray, that `KeepAlive` and `StartLimitBurst` behave as
   written, and that opening purlis from the Finder while it is in the background brings its
   window back. The seams are tested: the files each manager is given, the commands it is sent,
   the choice of supervisor, the window's handoff and its breakers, the quit's decision and the
   tray's words. A host that is a login item is tested over the session protocol. *D-655-8.*
9. **`session-host.log` is not rotated yet.** It holds the host's warnings only; its rotation is
   #6687. *D-655-9.*
