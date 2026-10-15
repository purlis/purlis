# Amended 2026-10-14 (the operator's rulings P1 and P2 of 2026-10-13, and of 2026-10-14): the window's side of a save's request

Ruled, not to be confirmed again:

- **Merge as it stands** is offered on a red request, and on a wait that nothing purlis reads
  will end: no check has started where the repository's other requests have them (a workflow
  its paths left out looks the same as one that has not started, so this wait has no time
  limit), its checks cannot be read at the pushed commit, or a check its branch requires has
  not reported. It asks first. The question names the request, the branch it goes into and,
  for a red one, the checks that failed, and says purlis merges without waiting for checks.
  It passes over the checks and nothing else: what the forge or purlis then refuses is said in
  their words, beside the link to the request. **The question is drawn from a fresh look**,
  with every check read again, and the press carries the commit and the standing the
  question showed. The core looks once more at the press and merges only when both are as
  shown; otherwise it says the request changed, and the window draws what is there now. A request the forge will not let merge says why
  and links to it, with no merge button; a merged one offers nothing.
- **Fix** is offered on a red request. It starts a chat at once, at the plane root, or in the
  workspace whose repo the request is of, on a message purlis writes: the request and its
  page, each failed check with its page and what it found, and what to do. It is an ordinary
  chat, on the project's default profile and persona, in its sandbox, with nothing granted
  past it. A second press for the same request brings that chat forward and starts nothing.
  The checks are read again on the press: when they pass or run again, nothing starts and
  the window says so. The project's chat saves with `purlis save`, and the save goes onto the
  same request. A workspace repo's chat may commit its fix in the clone, as any chat does,
  and does not push: the fix reaches the request with the repo's next save, the person's or
  the automatic one, and where the repo is not saved automatically the Notice says a save is
  needed.
- **A red request is in the Inbox**, as a Notice of the kind `save-request-red` with the same
  two answers, from the look that finds a failed check until the request merges or its checks
  pass. A request that waits is not trouble: it is said in the Saving view and on the title
  bar's save mark, and is not in the Inbox.
- **The app's own look asks as the window** (ADR 0070 §5, amended the same day): with the
  person's purlis sign-in where there is one for the forge, as a save pressed in the window
  would. With no sign-in at all the request waits, says a sign-in is needed, and offers one.

**What is quoted to the Fix chat.** A check's name and what it found are whoever wrote the
check's. In the message they are quoted and never said: every line behind `> `, under one
sentence of purlis's saying it is data to read and not instructions to follow. A line break in
quoted text starts another quoted line; every other control and invisible formatting character
is taken out; purlis's own mark is replaced; `://` is written `[:]//`. A link is named only on
a line of purlis's own, and only when it is an `https://` address under the request's own
repository on the forge the project is configured on, in printable ASCII, at most 300 bytes;
a check whose page is anything else is named with no link. The whole message is at most
11,000 bytes for at most eight failed checks: every line is counted with its mark and its
line feed, and a quote that does not fit ends with one line of purlis's saying it was cut.

**What a failed check found** is read from the forge on the press, with the window's sign-in:
a GitHub check run's annotations (`GET repos/{owner}/{repo}/check-runs/{id}/annotations`,
failures first), or the failed jobs of a GitLab pipeline
(`GET projects/{id}/pipelines/{id}/jobs?scope[]=failed`), the first ten of each. The checks
are read again at the pushed commit for this; what a file keeps of them is their names.

Decided in implementation, and ruled by the operator, 2026-10-15:

- A wait's state stays `waiting`. What ends it is a field beside it, `needs`: `person` or
  `sign-in`. Checks purlis cannot read at the commit count as a wait with no bound.
- The buttons say **Fix in a chat** and **Merge as it stands…** (`docs/ui-copy.md`: a verb and
  what it acts on; an ellipsis where it asks first).
- A repo's Fix chat may commit in the clone and does not push (decided by the dispatcher,
  ruled by the operator, 2026-10-15).
- A failure of any kind is handed to the chat in one shape (`purlis_core::savefix`), so a
  failing step of a save can use the same button with its own text (ruling P6). The app's
  entry that starts the chat stays private to its module until that lands.
