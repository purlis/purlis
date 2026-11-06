# Amended 2026-11-06: a Check reporter reports through `purlis check report` (#5410)

No check is run by purlis yet (ED-8 is Later), so a Check reporter is asked where results already
arrive, with two more options:

```
purlis check report <branch> <output> --reporter <extension> --status <n>
```

- **`<output>` is then the check's output, not SARIF.** purlis hands it, with the status the check
  exited with, to the approved extension that declares `check-reporter`, as its own process (ADR
  0041 as amended 2026-11-06), and records the SARIF it answers exactly as a SARIF file is recorded:
  each tool's results replace that tool's alone, at the branch's commit. `--status` is required with
  `--reporter`, since the status is half of what a reporter is handed.
- **A reporter that is not installed, not approved, turned off, changed since its yes or not a
  Check reporter is refused**, exit 1, and nothing is recorded.
- **An answer that is not SARIF 2.1.0, or a reporter that fails to answer, is its fault**: exit 1,
  saying what was wrong and naming the extension. The fault is kept in the branch's record beside
  its results (`docs/plane-format.md`), and the review draws it as a Notice naming the extension
  above the results. The results recorded before stay, so the review goes on. The reporter's next
  answer that is recorded takes the fault away.
- **In a sandboxed chat the app asks the reporter and makes the write**, reading only a file inside
  the named branch's folder, following no link, as it does for a SARIF file.
