# Amended 2026-10-13 (operator ruling R3): what a project expects is asked about up front

> **R3.** Kept: a committed project file declares, never grants.

With it the operator ruled that what a project declares it expects is asked about up front,
when a chat starts, and not discovered by a failure. A committed file still declares and never
grants (ruling 2), and each person allows on their own machine. What changes is **when the person is asked**. A
project that declared a socket said nothing on a new machine until a tool failed there, with
an error that often hid the cause. Now the declaration is read where the project is opened:

- **One Notice, once**, when a project opens (and when its settings change on disk) and its
  `[sandbox] expects` names something this machine has not allowed, for this project or for
  every project: "This project expects more than this machine allows", with a line for each
  item (its whole name, drawn inertly since a repository wrote it, and D-sock-6's words for
  what allowing it means), and, for one that cannot be allowed here now, why.
- **Allow on this machine** keeps each listed item that can be allowed, for this project on
  this machine, by the route a block's Allow takes: judged, audited, kept in this machine's
  file. Only what is declared, was shown and still waits is kept, whatever the window sends,
  and the answer is recorded for what was shown alone: an item declared since is asked about.
- **An Allow keeps what was shown, as it was shown** (review, 2026-10-13). The question is
  drawn once and may be answered much later, and a declared path can come to lead somewhere
  else in between. So the window sends each item as it drew it, the name an Allow would keep
  included, and nothing for one it drew as "cannot be allowed here now". An item is kept only
  while it still stands that way at the press. One that changed is neither kept nor recorded
  as answered: the Allow says so, and the question is drawn again as it stands.
  **Not now** is remembered on this machine (`expects_answered`); Settings › Sandbox has a
  row, "This project expects", that still offers the Allow. The Notice returns only when the
  project declares something new, or (#1927) when a path it declared comes to name another
  socket: an answer is kept with the name each item was shown under, so "Not now" to one
  socket is not "Not now" to whatever its path leads to later. A path that goes back to a
  socket already answered is not asked about twice.
- **Both answers are the window's alone** (`allow_project_expects`, `decline_project_expects`,
  window-only): no link and no chat can ask the question, answer it, or answer it in advance.
- **The words are purlis's; the names are data.** A repository wrote each name, so the Notice
  and Settings draw it apart from the sentence, with every character that draws as nothing
  written out, and each is judged as any grant's target is before anything is kept. A socket
  is named as the Allow would keep it, links resolved, with the path the project wrote beside
  it where that is another, so a harmless name that leads to another socket is shown for what
  it is. A socket path that cannot be said plainly (a character that draws as nothing, or longer than a path
  is) declares nothing. A project declares at most 32 sockets and services together, so the
  question is always one a person can read whole; what a file lists past that declares nothing.
- **A chat's pane says it from the start**: a chat that starts while something declared is
  neither allowed nor declined says so in one line, and where to allow it. The line counts
  what is missing and names none of it: it is a sentence of purlis's, whole. A chat started
  after the Allow has it; one already running is listed on the "Sandbox changed" Notice with
  Restart, which now counts sockets and services among what a sandbox is made of.
- **Policy.** Under `socket-grants: false`, or `allow-scopes` without `you`, the Notice says
  who locked it and offers nothing but "Got it".
- **The certificate check is not part of this**: it is no grant any more (R1 above), so it is
  never declared, listed or asked about.
- **The onboarding steps have no step that introduces the sandbox**, so none was added there;
  the Notice appears when the project a person just made or got opens.
