# Spike T1.1-51 (#6898, Q295): can purlis read seatbelt's denial reports for a chat's own processes?

**Answer: partly.** Yes for an admin user, promptly, using `log stream` with a per-chat tag; in
every measured run the other, cooperating sandboxed process's denials did not match the chat's tag
(spoofing and cross-chat attribution were **not** measured, see "What was not measured"). No for a
standard (non-admin) user: macOS refuses both `log stream` and `log show`. The first few refusals
of a new process were sometimes not reported. A rapid run of identical refusals is not reported
line by line: seatbelt folds identical refusals into a later "N duplicate report(s)" line. That
line **does carry the tag** (on a second line of the same event, like every report), so
`ENDSWITH "<tag>"` matches it, and a receiver that parses the "N" accounts for every identical
refusal: in 11 of 12 runs of the burst, the lines that stood alone plus the refusals the folded
lines stand for came to exactly 100 of 100 (the twelfth: 101). An earlier version of this
document said the folded lines carried no tag; that was a fault in the probe, which looked only at
the first physical line. Ticket 52 can build the receiver for admin users; the standard-user case
needs the person's ruling.

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
second, many processes) was measured. Final run, on head `70b393f22`:
[44926078905](https://github.com/purlis/purlis/actions/runs/44926078905) (the commits after it
change this document only). Its folded-summary numbers are the only ones that count: its judge
joins a record with its continuation line, and it runs on the ENDSWITH-only cases (A) as well as
on F. The earlier runs
[44915286114](https://github.com/purlis/purlis/actions/runs/44915286114),
[44916069923](https://github.com/purlis/purlis/actions/runs/44916069923),
[44917131228](https://github.com/purlis/purlis/actions/runs/44917131228) and
[44921998297](https://github.com/purlis/purlis/actions/runs/44921998297) agree on latency,
strays and missing reports; their "0 folded lines carried the tag" was the probe's artefact and is
withdrawn. The Claude Code profile was not probed here: that is ticket 10's leg.

## Candidates

| Candidate | Result |
|---|---|
| `(deny … (with message "<tag>"))` and `eventMessage ENDSWITH "<tag>"` | **Holds** (admin). Each report is `kernel (Sandbox) Sandbox: perl(pid) deny(1) file-write-create <path>` with the tag at the end (on a second line of the event); so is a folded "N duplicate report(s)" line. In the runs, the other process's reports never matched (0 of ~210 deny lines per case); that process cooperated, and its tag was built not to match (see "What was not measured"). |
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

Seatbelt does **not** report each repeat as its own line. Of 100 identical refusals 10 ms apart,
84 to 98 lines named the file in the final run (73 to 99 in the earlier runs), and the first of a
run was always reported. The rest were folded: see below. Counting a folded line for the N
refusals it stands for, the burst came to exactly 100 of 100 in 11 of the 12 runs of the final
run's cases A and F (101 in one, cause not found), so no identical refusal went unreported. Of 20
identical refusals 0.5 s apart, 20 were reported, as separate lines, in every case of the final
run. A receiver that counts lines will see fewer than the refusals; one that adds the "N" of each
"N duplicate report(s)" line sees all of them. **This goes to the person**, as the ticket asks:
ticket 52's repeat test should expect the sum of the lines and their folded counts to equal the
refusals made (after the last fold arrives), not one line per refusal.

### Folded summary lines

Seatbelt (the kernel) folds identical refusals into a summary line. As the stream prints it, the
event is two physical lines, the second being the tag (the working directory is redacted; this is
the first record of macos-15 case A in run 44926078905):

```
2026-11-21 16:44:02.146 E  kernel[0:2324] (Sandbox) 1 duplicate report for Sandbox: perl(2027) deny(1) file-write-create <chat dir>/rep
spk3169024862
```

**The folded line carries the tag, and `ENDSWITH "<tag>"` matches it.** Case A uses only that
predicate. In its 8 runs (both macOS versions, 3 plain and 1 loaded each) the kernel wrote 16 to 28
folded records for the chat's own directory, standing for 18 to 34 refusals, and **every record
ended with the tag** (8 of 8 runs; the same in the 4 F runs, whose predicate also matches
"duplicate report"). In those 12 runs the lines that stood alone plus the refusals the folded
records stand for came to 100 in 11 and 101 in one. A folded record is one line whatever its N, so
a line count undercounts, but the N is in the line.

Timing: in the 8 A runs, which wait only 12 s after the child ends, the burst already summed to
exactly 100 in all 8, so the folds arrived within that wait there. In earlier run 44917131228
(macos-15, case E) one fold arrived about 25 s after its process ended, so a receiver should keep
reading after the process ends and not close on the process's exit.

What the folded line does not carry is a time per refusal: it gives a pid, a path and a count.

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
allowed (a folded repeat arrives as an "N duplicate report(s)" line that ends in the same tag, so the predicate sees it: add the N to the count). A filter on the pid would miss a
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

