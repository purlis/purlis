# Amendment, 2026-10-01: the Tauri log directory also holds the diagnostic log (#647)

§1 says the Tauri application-data and log directories hold only the shims and the git hooks,
rebuilt at every launch, and the panic log. **The log directory now also holds the app's
diagnostic log**, `<app log>/charter.<YYYY-MM-DD>.log`, written by `charter_core::applog`
(FD-8, #647). It is **Machine, device-bound, transient**, as `panics.log` (row 57) is, and
`docs/plane-format.md` has its row. A new file starts each day and the newest seven are kept.
FR-10 does not back it up: it records what happened on this machine, and only a person
reads it. It is neither the audit nor telemetry (O1, ADR 0075). Nothing else in this record
changes.
