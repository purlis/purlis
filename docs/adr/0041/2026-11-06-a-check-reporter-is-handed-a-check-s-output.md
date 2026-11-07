# Amended 2026-11-06: a Check reporter is handed a check's output, and runs as the operator (#5410)

The **Check reporter** capability (ADR 0117, ADR 0053 as amended 2026-11-06) is an extension's
program that purlis hands one check's output and exit status, and whose stdout it reads as SARIF.
What that changes in this record's threat model:

- **It is watched, not confined**, like every extension but a studio one. It runs as the operator,
  as its own process through the executor, never inside the window. It is handed no plane path, so
  anything it changes in the plane while it answers is reported with the answer.
- **It is started only by a report**: `purlis check report <branch> <output> --reporter <id>
  --status <n>`, typed by the operator, a script, CI or a chat. The approval prompt says so.
- **A chat's report is run by the app, not in the chat's sandbox.** The chat's sandbox does not
  write the workspace's `pieces/` (ADR 0067 §2), so the app makes the write, as it does for a SARIF
  file, and it asks the reporter as it asks every extension, as the operator. What the chat can
  choose is bounded: which approved reporter, the status, and a file **inside the branch's folder,
  opened following no link** and at most 8 MiB, so a chat cannot have the app hand a reporter a file
  the chat's sandbox denies it. A reporter the operator never approved, turned off or changed since
  its yes is refused before anything runs.
- **What it answers is data.** purlis reads it as SARIF 2.1.0 and nothing else. Anything that is
  not SARIF is kept as the reporter's fault, which the window draws as a Notice naming it, and the
  results recorded before stay.

**Built by #5410.**
