# A set's folder goes once no running chat uses it

**2026-10-21**, ticket #2856 (SK-24), amending [the routes carry the plane's skills](2026-10-20-the-routes-carry-the-plane-s-skills.md)
under [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md)
and the operator's spec #2762.

Each skill set gets its own folder of links, `<data>/skill-sets/<digest>/`, and an edit to a
skill makes a new set. Folders no chat uses would pile up, so purlis removes them, and never one
a running chat was handed.

**A running chat is noted, by its process.** As the host arms a chat, the folder it hands over
travels in the chat's start as `PURLIS_SKILL_SET`. Where the chat's terminal is composed
(`chatspec::spec`, which every host uses), that variable is taken out of the environment, so the
program never sees it, and becomes a hold (`session::Spec::holds`). Once the program has started,
the terminal writes `<data>/skill-sets/.held/<digest>/<pid>`. It removes that file once the
program has been ended. A note counts while its process runs and had started by the time the note
was written (`process::could_be_recorded`). A note left by an app that crashed names a process
that has gone, or one that started later, and is removed.

**A tidy removes what is not noted and not recent.** `skills::tidy` removes a set's folder that no
live note names and that was not laid out or found within the hour, `TIDY_GRACE`. That hour
covers the time between the host laying a folder out and the terminal noting it. `lay_out`
touches a folder it finds complete, so the grace counts from the last chat start that used it.
A tidy and a lay-out take turns on `<data>/skill-sets/.lock`. A lay-out takes it shared. A tidy
only tries it, and looks at nothing while a chat is starting, so a tidy never makes a start wait.
The app tidies as it starts, and five seconds after a chat closes, on a thread of its own. What it
removed and what it could not are said in the log, and a failure never holds up the app's start.

**Only purlis's own names are removed, and nothing through a link, even one swapped in after a
check.** A tidy works from handles, never back through a path it has checked. It opens
`skill-sets`, `.held`, each `.held/<digest>` and each folder it removes as a folder itself
(`O_DIRECTORY | O_NOFOLLOW`, rustix's `openat`), lists through the handle, and removes relative
to it (`unlinkat`). It removes a real folder named by a digest (64 lowercase hex), or what an
unfinished lay-out left (`.<digest>.<suffix>`). In `.held/<digest>` it only removes files whose
name is a process id; anything else stays and goes in the log. A link at `.held` or at a
`.held/<digest>` keeps the folders it may speak for, since notes that can't be read may name a
running chat. A folder that turns into a link between its check and its removal is refused. A
link inside a set's folder is removed as a link, so the plane's skills it points to stay. The
terminal opens a hold, and the folder it sits in, the same way before the program starts. It
makes its note with `O_CREAT | O_EXCL | O_NOFOLLOW` relative to that handle, and removes it
relative to that handle too. `.lock` is never opened through a link.

**Nothing is removed on Windows yet**: whether a process still runs can't be asked there
(`process::alive`), so a tidy there does nothing until it can.

## What was rejected

- **Reading every process's arguments and environment.** opencode is handed the folder in its
  environment. Reading that for every process takes a new `unsafe` block on macOS, or a `ps` run
  whose output can't tell a process it couldn't read from one that doesn't use the folder.
- **A lease held by the window.** The session host keeps running a window's chats for ten minutes
  after the window crashes. The next window's start would then see a dead holder and remove
  folders that running chats still use.
- **A field in the session protocol's launch.** It would carry the same fact as the variable, at
  the cost of a protocol change and some forty call sites. The variable already travels in the
  launch's `set`.
- **Removing folders by age alone.** A chat can run for days.
