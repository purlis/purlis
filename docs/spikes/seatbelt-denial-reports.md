# Spike T1.1-51 (#6898, Q295): can purlis read seatbelt's denial reports for a chat's own processes?

**Answer: partly.** Yes for an admin user, promptly and with no other process's denials, using
`log stream` with a per-chat tag. No for a standard (non-admin) user: macOS refuses both
`log stream` and `log show`. The first few refusals of a new process were sometimes not reported,
and a rapid run of identical refusals is not reported in full. Ticket 52 can build the receiver for
admin users; the standard-user case and the exactness of counts need the person's ruling.

## How it was measured

`tools/spikes/seatbelt-denial-reports.sh` is run by `.github/workflows/seatbelt-denial-reports-spike.yml`
on `macos-15` and `macos-26` (15.8.2 and 26.7.2), once as the runner's admin user and once as a
fresh standard user (`sysadminctl -addUser`, group list: `staff everyone localaccounts`). A
`perl` child runs under `sandbox-exec` with `(deny file-write* <modifier> (subpath <chat dir>))`
and tries 100 distinct files, then the same file 100 times (10 ms apart), then the same file 20
times (0.5 s apart). A second sandboxed process, with a different tag, does the same at the same time.
The receiver is `log stream --style compact --predicate <per-chat predicate>`, stamped on arrival.
Also run: a CPU and `logger` flood (4 `yes` loops, two `logger` loops), and a broad predicate
(`sender == "Sandbox" OR process == "kernel"`) for comparison. Final run:
[44917131228](https://github.com/purlis/purlis/actions/runs/44917131228); earlier runs
[44915286114](https://github.com/purlis/purlis/actions/runs/44915286114) and
[44916069923](https://github.com/purlis/purlis/actions/runs/44916069923) agree. The Claude Code
profile was not probed here: that is ticket 10's leg.

## Candidates

| Candidate | Result |
|---|---|
| `(deny … (with message "<tag>"))` and `eventMessage ENDSWITH "<tag>"` | **Holds** (admin). Each report is `kernel (Sandbox) Sandbox: perl(pid) deny(1) file-write-create <path>` with the tag at the end. The other chat's reports never matched (0 of ~210 deny lines per case). |
| Plain deny, with the tag in a path component and `eventMessage CONTAINS "<tag-dir>"` | **Holds** (admin), same numbers. Needs no profile change, but only covers paths under the tagged directory. |
| `(with report)` | **Not valid on a deny:** `sandbox compile: report modifier does not apply to deny action`. A deny already reports. |
| `(with no-report)` (control) | Zero reports, so the harness sees what it should. |
| `(trace "<file>")` | Compiles; its output was not examined. |
| Per-chat unified-log subsystem | Not applicable: the denial is logged by the kernel as `Sandbox`, and a chat sets no subsystem on it. Not measured. |
| Broad predicate (comparison) | Also carries other processes' denials (about 200 lines per case from the other chat, plus the system's, e.g. `triald`). |

## Numbers (admin, both macOS versions, 100 distinct refusals per case)

- **Latency** from the refusal to the line arriving: median under 1 ms, p95 1 to 3 ms, worst 37 ms
  (one 0.3 s outlier on `macos-26` with the broad predicate under load). Load did not move it.
- **Strays:** 0 of every targeted run's deny lines came from outside the chat.
- **Duplicates:** 0.
- **Missing:** in about half of the cases (both tags, both OSes, loaded or not) the first 3 to 5
  distinct refusals (`d1` to `d4` or `d5`) of the new process had no report; in the others none was
  missing. Nothing after the first five was missing in about 2,000 distinct refusals. The cause was not found.
  A receiver therefore cannot assume that the first refusal of a chat is reported.

## Repeated identical refusals (Q299)

Seatbelt does **not** report a repeat only once. Of 100 identical refusals 10 ms apart, 73 to 99
were reported (usually 82 to 99); the first of a run was always reported. Of 20 identical refusals
0.5 s apart, 20 were reported in 16 of 19 cases and 19 in three (all on `macos-15`, two of them loaded). A repeat is seen,
but a test that needs every one of a rapid run will fail. **This goes to the person**, as the ticket
asks: ticket 52's repeat test should expect at least one report per burst, not one per refusal, unless
the person rules otherwise.

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
allowed. A filter on the pid would miss a chat's children; a profile-wide tag should catch them (children were not measured here).
