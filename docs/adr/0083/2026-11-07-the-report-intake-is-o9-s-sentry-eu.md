# Noted (2026-11-07): the Report intake is O9's Sentry EU, listed in the network log (#5685)

[ADR 0118](../0118-an-unread-report-goes-only-to-the-private-report-intake-never-to-a-public-tracker.md)
follows this record and changes none of it. Crash reports, and Report drafts sent unread under
**Always send**, go to the **Report intake**: the Sentry SaaS project in the EU that ruling O9
names, a third party and not a purlis server. §9 still holds, and every send to it is listed in
the local network log (X50). Crash reports are not product telemetry: OB-12 stays a separate
opt-in. The choice O4 gives an org's admin, of where members' data goes, does not yet reach
reports; it waits for GT-CLOUD and TM-2.
