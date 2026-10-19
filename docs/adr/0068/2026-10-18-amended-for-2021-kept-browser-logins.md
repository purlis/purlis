# Amended for #2021, kept browser logins, 2026-10-18

The person ruled on 2026-10-18 that a profile keeps its browser logins sealed on this device
(ADR 0095, ruled 2026-10-18). The amendments above are left as accepted, and this note adds to
them.

1. **Session protocol 1.27, a minor.** A browser's start may name the kept login it opens from
   (`kept`: a profile, a login and the login's page), and `log_in` the kept login the browser
   keeps once it ends (`keep`). Both name, never carry, a value. A start that names `kept` is
   taken from the window's link alone (`Power::LogIn`), as `log_in` is, and a relaying host sends
   neither on: kept logins never leave this device. Recorded in the protocol's fixture.
   *Decided in implementation, D-2021-5, to be confirmed.*
2. **The binary does the keeping**, as it performs logins: it opens a store into the browser's
   fresh folder before its engine starts, binds the browser to the login's site at once, and
   writes the store back once the engine has ended and before the host removes the folder. The
   host links no keyring code. *Decided in implementation, D-2021-6, to be confirmed.*
