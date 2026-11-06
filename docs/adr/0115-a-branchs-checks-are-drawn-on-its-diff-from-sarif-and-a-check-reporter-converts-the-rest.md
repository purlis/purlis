# A branch's checks are drawn on its diff from SARIF, and a Check reporter converts the rest

**Accepted 2026-11-05** by the operator (editor grilling, rulings E-Q5, E-Q12 and E-Q15). Its
concept is **Workspace**: the diagnostics are part of a review of a chat's branch. It builds on
ADR 0081 (as amended 2026-11-05), ADR 0084 and ADR 0053.

While reviewing an agent's change, the operator wants to know whether it breaks the build, and
where. ADR 0081 keeps purlis from running a language server. So the diagnostics come from the
branch's own checks (typecheck, lint, tests; ED-8, ED-9), and they are drawn on the diff in the
light editor as data.

**purlis reads one format: SARIF 2.1.0**, the OASIS standard for static-analysis results. A
result's file and region are mapped onto the comparison's lines; a result on a line the diff
does not show is listed with its file, not dropped. purlis has no parser for any one tool.

**A tool that does not write SARIF is converted by an extension** that declares the **Check
reporter** capability: purlis hands it the check's output, and it answers SARIF. Like every
capability, it is declared in the manifest and approved (ADR 0053), and it runs as the
extension's own program (ADR 0041), never inside the window.

Results arrive by `purlis check report`: see
[its amendments](0115/), of 2026-11-05.

## Considered options

- **Also parse the common `file:line:col: message` line in the core.** It covers more tools with
  no extension, but it is a guess at a format with no specification, and every tool's variant
  would become the core's to keep.
- **A language server per branch, read-only, while reviewing.** ADR 0081's "No LSP" stands; see
  its amendment of 2026-11-05.

## Consequences

- A check whose tool writes SARIF (eslint's formatter, CodeQL, semgrep, clippy through
  `clippy-sarif`) needs nothing more.
- The Check reporter is the first capability that grows purlis's review through extensions,
  the operator's "scale through extensions" as ruled in E-Q4: more capabilities, each approved.
