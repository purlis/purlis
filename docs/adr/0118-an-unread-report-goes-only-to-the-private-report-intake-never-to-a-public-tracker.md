# An unread Report goes only to the private Report intake, never to a public tracker

**Accepted 2026-11-07** by the operator, in the grilling of the spec *people and agents report
purlis's bugs* (#5683; questions Q2b, Q3, Q12 and Q25, each on the recommended answer), recorded
for #5685. It amends [ADR 0059](0059-a-report-is-filed-only-under-the-digest-of-the-draft-that-was-shown.md)
for the Always-send route only, and follows
[ADR 0083](0083-telemetry-is-one-otel-pipeline-on-the-device-and-only-the-signed-updater-calls-charter.md)
without changing it. The words are CONTEXT.md's: **Report**, **Report draft**, **Always send**,
**Report intake** and **Crash report**.

ADR 0059 files a Report only under the digest of the draft that was shown, so what is filed is
what the person saw. The spec asks for more reports than a person will stop to read: agents now
write Report drafts of purlis's bugs, and a person may say once, with **Always send**, that drafts
may leave without being read. A draft an agent wrote is prose about the person's work. Nobody has
read it, and the scrub catches only what purlis can identify (ADR 0059). On a public tracker it
would be public, under the person's name, for good.

## The decision

**A Report that nobody has read may go to one place only: purlis's private Report intake, never
to a public tracker.** The Report intake is O9's crash-report service, Sentry SaaS in the EU (ADR
0083, ruling O9), in a project only purlis's maintainers can read.

- **Always send sends to the Report intake, and nowhere else, ever.** It is the person's standing
  yes for unread drafts, so it never reaches GitHub, whether or not the person is signed in to
  it. A maintainer promotes what the intake receives to purlis/purlis issues; nothing an agent
  wrote becomes public without a maintainer reading it first.
- **A draft the person read and filed keeps ADR 0059's rule.** Filed by a person signed in to
  GitHub, it goes to purlis/purlis as them, under the digest of what they were shown. Filed by a
  person who is not, the same read draft goes to the Report intake.
- **Crash reports go to the Report intake** with their own consent, as O9 rules.
- **Always send has limits that do not depend on the agent.** A draft waits a few seconds with
  Undo before it leaves; at most five go unread a day from a machine, after which drafts ask
  again; it can be turned off at any time; and every report sent is listed, byte for byte, on the
  machine.
- **A project can forbid Always send for its members.** A member still files drafts after reading
  them.

**ADR 0083 is followed, not changed.** The Report intake is a third party that O9 names, not a
purlis server, so §9's *"no network calls to Charter's servers without an account"* still holds,
and every send to it is listed in the local network log (X50).

**Known gap: a team cannot yet send its members' reports to its own intake.** O4 has an org's
admin decide where members' telemetry goes; the same choice for reports needs an org, which waits
for GT-CLOUD and TM-2. Until then a team has one lever, the project rule above.

## Considered options

- **Always send to GitHub as the person.** Rejected. It would file an agent's unread prose about
  the person's work, possibly about a client, on a public tracker under their name. ADR 0059's
  digest exists so that never happens.
- **No Always send: every draft waits for the person to read it.** Rejected. Drafts no one has
  time to read are reports no one sends, which is the problem the spec starts from.
- **A purlis-hosted intake.** Rejected. It would be a call to purlis's own servers without an
  account, which ADR 0083 §9 forbids, and a service to run. O9 already ruled Sentry EU in.
- **A GitLab project or another forge as the intake.** Rejected (Q3): it is a public tracker or
  one more account to hold, and the person without a GitHub sign-in is exactly the one who has
  none.

## Consequences

- Two routes leave a machine with a Report: GitHub as the person, only for a draft they read; and
  the Report intake, for everything else. No third route exists.
- The intake's address (Sentry's DSN) is built into the binary. It is public by design: it lets
  anyone send to the intake and no one read it.
- The maintainers must read the intake. A daily steward routine promotes its reports to issues
  labelled `reported`; a report it does not promote is never public.
