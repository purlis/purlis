# Amended for #2900, a purge takes the engine profile the person added, 2026-10-22

The Notice about the browser engine's sandbox offers a command that writes
`/etc/apparmor.d/purlis-engine-here` (#2835, D-2835-2). Nothing removed it when purlis was.

1. **A purge of the `.deb` unloads and removes it; a remove keeps it.** A remove leaves a
   package's configuration behind and a purge removes it, and this file is configuration of
   purlis's own: written with purlis's command, for purlis's engine, and read by nothing else.
   The postrm removes it only where it is still that profile (a line `profile
   purlis-engine-here`), so a file of the person's own by that name is left alone. *D-2900-1,
   decided in implementation, to be confirmed.*
