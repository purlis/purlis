# Spike T1.1-51 (#6898, Q295): can purlis read seatbelt's denial reports for a chat's own processes?

**Answer: partly.** Yes for an admin user, promptly, using `log stream` with a per-chat tag; in
every measured run the other, cooperating sandboxed process's denials did not match the chat's tag
(spoofing and cross-chat attribution were **not** measured, see "What was not measured"). No for a
standard (non-admin) user: macOS refuses both `log stream` and `log show`. The first few refusals
of a new process were sometimes not reported, and a rapid run of identical refusals is not reported
in full: seatbelt folds identical refusals into a later "N duplicate reports" summary line, and
that line carries **no** tag, so a tag-based receiver loses folded repeats for good. Ticket 52 can
build the receiver for admin users; the standard-user case and the exactness of counts need the
person's ruling.

## How it was measured

`tools/spikes/seatbelt-denial-reports.sh` is run by `.github/workflows/seatbelt-denial-reports-spike.yml`
on `macos-15` and `macos-26` (15.8.2 and 26.7.2), once as the runner's admin user and once as a
fresh standard user (`sysadminctl -addUser`; its full group list: `staff everyone localaccounts _lpoperator com.apple.sharepoint.group.1`). A
`perl` child runs under `sandbox-exec` with `(deny file-write* <modifier> (subpath <chat dir>))`
and tries 100 distinct files, then the same file 100 times (10 ms apart), then the same file 20
times (0.5 s apart). A second sandboxed process, with a different tag, does the same at the same time.
The receiver is `log stream --style compact --predicate <per-chat predicate>`, stamped on arrival.
Also run: a CPU and `logger` flood (4 `yes` loops, two `logger` loops; cases A and B only), a
broad predicate (`sender == "Sandbox" OR process == "kernel"`; case E, never under load) for
comparison, and a folded-summary case (F, below, admin only). The refusals come at about 100 per
second from one process, on a machine with that CPU and logger load: no denial storm (thousands per
second, many processes) was measured. Final run:
[44921998297](https://github.com/purlis/purlis/actions/runs/44921998297); the earlier runs
[44915286114](https://github.com/purlis/purlis/actions/runs/44915286114),
[44916069923](https://github.com/purlis/purlis/actions/runs/44916069923) and
[44917131228](https://github.com/purlis/purlis/actions/runs/44917131228) agree, and
[44920118675](https://github.com/purlis/purlis/actions/runs/44920118675) (admin legs only; the
standard-user step hit the old 20-minute limit) also measured case F. The Claude Code
profile was not probed here: that is ticket 10's leg.

## Candidates

| Candidate | Result |
|---|---|
| `(deny … (with message "<tag>"))` and `eventMessage ENDSWITH "<tag>"` | **Holds** (admin). Each report is `kernel (Sandbox) Sandbox: perl(pid) deny(1) file-write-create <path>` with the tag at the end. In the runs, the other process's reports never matched (0 of ~210 deny lines per case); that process cooperated, and its tag was built not to match (see "What was not measured"). |
| Plain deny, with the tag in a path component and `eventMessage CONTAINS "<tag-dir>"` | **Holds** (admin), same numbers. Needs no profile change, but only covers paths under the tagged directory. |
| `(with report)` | **Not valid on a deny:** `sandbox compile: report modifier does not apply to deny action`. A deny already reports. |
| `(with no-report)` (control) | Zero reports, so the harness sees what it should. |
| `(trace "<file>")` | Compiles; its output was not examined. |
| Per-chat unified-log subsystem | Not applicable: the denial is logged by the kernel as `Sandbox`, and a chat sets no subsystem on it. Not measured. |
| Broad predicate (comparison) | Also carries other processes' denials (about 200 lines per case from the other chat, plus the system's, e.g. `triald`). |

## Numbers (admin, both macOS versions, 100 distinct refusals per case)

- **Latency** from the refusal to the line arriving: median under 1 ms, p95 1 to 3 ms in most
  cases, with p95 17 ms and a maximum of 53 ms seen in run 44916069923 on `macos-26`. The broad
  predicate (case E) was never run under load. Load (cases A and B) did not move it visibly.
- **Strays:** 0 of every targeted run's deny lines came from outside the chat, for the one
  cooperating second process measured.
- **Duplicates:** 0.
- **Missing:** in about half of the cases (both tags, both OSes, loaded or not) the first 3 to 5
  distinct refusals (`d1` to `d4` or `d5`) of the new process had no report; in the others none was
  missing (the earliest runs did not print which files were missing, so that is from the later
  runs only). Nothing after the first five was missing in about 1,800 distinct refusals. The cause was not found.
  A receiver therefore cannot assume that the first refusal of a chat is reported.

## Repeated identical refusals (Q299)

Seatbelt does **not** report a repeat only once. Of 100 identical refusals 10 ms apart, 73 to 99
were reported (usually 82 to 99), in one run 100 of 100; the first of a run was always reported. Of
20 identical refusals 0.5 s apart, 20 were reported in 15 of 18 cases and 19 in three (all on
`macos-15`, two of them loaded). A repeat is seen,
but a test that needs every one of a rapid run will fail. **This goes to the person**, as the ticket
asks: ticket 52's repeat test should expect at least one report per burst, not one per refusal, unless
the person rules otherwise.

### Folded summary lines

Seatbelt (the kernel) folds identical refusals into a summary line such as
`kernel (Sandbox) 3 duplicate reports for Sandbox: perl(34685) deny(1) file-write-create <path>`.
It is not always deferred: in the later runs most folds arrived within a few seconds of the burst,
and in run 44917131228 (macos-15, case E) one arrived about 25 s after its process ended, past the
probe's 12 s wait.

**The folded line carries no `(with message …)` tag.** Case F waits 60 s after the child ends and
uses the predicate `eventMessage ENDSWITH "<tag>" OR eventMessage CONTAINS "duplicate report"`. In
the final run (44921998297, two cases on each of `macos-15` and `macos-26`) the kernel wrote 21 to
28 summary lines for the chat's own directory per case, standing for 28 to 38 refusals, and **0 of
them ended with the tag**. Run 44920118675 agrees (16 to 30 lines, 28 to 44 refusals, 0 tagged).
A summary line therefore does not match `ENDSWITH "<tag>"`, so candidate
A loses folded repeats for good. A receiver can still see them by also matching `duplicate report`
and the chat's path, but then it is back to matching on the path (candidate B's weakness), a line
that names a pid and a path but no tag, and it counts the refusals folded in, not their times.

## Standard user

`log stream`: `log: Must be admin to run 'stream' command`. `log show`: `log: Could not open local
log store: Operation not permitted`. The Sandbox lines live in the local log store, so a non-admin
user has no way found here to read them, with or without a tag, and Full Disk Access was not tried.
Every cell as that user reported 0 of 100. Most Macs' first user is an admin, so the receiver
covers that case. **Where the person is a standard user, no candidate here works**; the person
decides whether ticket 52 leaves that user unverified, asks for admin, or looks for another channel.

## What ticket 52 builds on

Run `log stream --predicate 'eventMessage ENDSWITH "<per-chat tag>"'` from the host, put
`(with message "<tag>")` on the deny rules of purlis's own profile, start the stream before the
chat's process, and treat a report as evidence of a refusal and a missing one as unknown, not as
allowed (a folded repeat is also unknown to this predicate). A filter on the pid would miss a
chat's children; a profile-wide tag should catch them (children were not measured here).

Requirements for ticket 52, because the predicate filters on `eventMessage` only and any local
process can write a log line that ends in the tag:

- a **sender filter**: accept only lines whose sender is the kernel / `Sandbox` (for example add
  `sender == "Sandbox"` or `process == "kernel"` to the predicate, and check it again when parsing);
- **pid and line-structure parsing**: parse `Sandbox: <process>(<pid>) deny(<n>) <operation> <path>`
  and accept the line only if the pid is the chat's process or one of its descendants and the path
  is a path the chat could have touched; reject anything else, including a line that merely ends
  in the tag.

## What was not measured

- **Spoofing:** whether a local process can write a line ending in a chat's tag to the unified log
  (it can write to the log; the tag was not tried against a real attacker) and so forge or hide a
  report. The recommended predicate would accept it.
- **Cross-chat attribution:** the only other process was one cooperating process whose tag was
  built not to match. Two chats that share a tag, a tag that is guessed, or a child that outlives
  its chat were not tried.
- A denial storm (thousands of refusals a second, many processes), the Claude Code profile (ticket
  10's leg), children of the chat's process, and `(trace)` output.

