# A pane link starts from where the chat is now, never widening its home

**Accepted 2026-11-18**: #6367 (milestone M73, Links in a pane), left over from #6216 (PR #6316).
The steward's choice, recorded so the next kind of link that reads a folder reads it the same way.

## Where purlis was

A relative path in a pane (`src/main.rs:42`) resolved against the folder the chat **started**
in, because nothing told purlis when the chat's folder changed. After a `cd`, the path linked a
file in the wrong folder, or none. The same start folder also did three other jobs: it said
what the chat's **home** is (`panelinks::home_of`: its workspace, or the project), which decides
whether a file opens or asks first; it was one of the folders a hover may look in although they
are guarded (macOS's Documents, a network volume); and an OSC 8 link's words were read from it.

## The decision

### 1. Two sources of "where it is now", and the order they are taken in

- **The harness's hook.** Claude Code puts `cwd` on every hook's payload. The harness's adapter
  reads it (`HarnessAdapter::reported_folder`, `None` by default), as it reads the permission
  mode, and the report carries it as `Detail::folder`. The app believes it only from the chat's
  own harness (`panelinks::folder::of_its_own_harness`: the conversation the board holds, no
  sub-agent), keeps it in memory per chat, and forgets it as the chat starts again or closes. It is
  never spooled or written down.
- **OSC 7 from the pane's program** (`ESC ] 7 ; file://host/path ST`, which shells send at their
  prompt). The window keeps the last one its terminal was sent, cut to 2 KiB, and hands it with
  each resolve and open as `LinkFrom::folder`, as it was said. The core reads it
  (`folder::said_by_osc7`): only `file:` naming no host, `localhost` or this machine (same name
  without case once `.local` is off, or a bare name that is this machine's first label); no
  query, fragment, user or port; a plain absolute path with no `.` or `..` part and no control
  character.

The hook's folder is taken first, then OSC 7, then the start folder (`folder::now_in`). A harness
that reports its folder knows where it is; OSC 7 in a harness's pane would come from something
that printed it, which may be a file the agent showed.

### 2. Where it is now moves one thing only: where a relative path starts

`Pane::now_in` is used by `Pane::starts_from` alone: the file kind's `joined`, and an issue's
number, whose remote is read only from a folder in the project. **OSC 8's check of a marked
link's words still reads them from the start folder**: a program that marks a link could also
say, by OSC 7, that it moved to where its words name the target, and so quiet the ask they raise. **The home is still judged from the start folder**, and so is the list of
guarded folders a hover may look in because they are the chat's. So a folder the chat moved to
outside its workspace makes a relative path there exactly what the same path typed in full is: a
link that asks first ("outside smart-ide"). One in a guarded folder is never looked in. A folder
on another host is none. Neither source can make anything open without an ask that would have
asked before; the pane could already send any absolute path, and a relative path joined to a
folder is one of those.

## What was not chosen

- **The latest report wins, whichever source.** It needs the window and the app to agree on time
  for two clocks' events, and lets anything printed in a harness's pane override its harness.
- **Moving the home with the folder.** A chat that `cd`s into another workspace would open that
  workspace's files without asking: the folder would widen what opens, which OSC 7, a program's
  own words, must never do.
- **A command the window calls when OSC 7 arrives**, keeping the folder in the app. One more
  command for a value the window already holds and the core reads as untrusted either way.
- **Reading the folder of the chat's process from the system** (`lsof`, `/proc/<pid>/cwd`). A
  look at another process per hover, and a harness's tool runs in a child whose folder is not
  its parent's.

## Consequences

- A relative path follows a `cd`: at once by OSC 7 in a shell's pane, and in a Claude Code chat
  from its next report (a `Stop`, a `Notification`, a prompt).
- Codex and opencode keep their start folder until their adapters are measured to send a `cwd`.
- **After a `cd`, a relative word over a marked link asks.** An OSC 8 link whose words are a
  relative path (`src/lib.rs`) is compared with its target from the start folder, so once the
  chat has moved the words lead to the target only from the new folder and the link asks first,
  showing both. This is the safe side: it asks where it could have opened, never the reverse.
- A hover answered before a move may show the old folder's file for up to its answer's life (5 s
  for a file); opening always resolves again, from where the chat is then. The window lets go of a
  pane's answers when its program sends a new OSC 7.
