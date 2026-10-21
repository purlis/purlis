# Amended for #2207, the engine's first download is shown as it goes and can be cancelled, 2026-10-21

The first browser on a machine downloads the pinned engine, about 150 MB, inside its start. The
person saw nothing while it ran, could not stop it, and a failure said only that the start
failed.

1. **The session host tells each window how far the download has come** (session protocol 1.37,
   `engine_download`: the browser's `program`, the bytes `done`, and the `total` where the server
   said it). It tells a few times a second at most (`DOWNLOAD_TOLD_EVERY`, 250 ms), once as it
   begins and once at its end. Only a link that speaks 1.37 and holds the browser power is told:
   an older link never receives the push.
2. **A window cancels a download** with `cancel_engine_download { program }`, a window-only
   command of the browser power. The download stops at its next read, nothing of it is kept in
   the engines folder, and the start is refused in words that say it was cancelled and how to
   download it again.
3. **A download that fails says what to do**: check the connection and open the browser again,
   or, where the archive is not the one purlis pins, that it was not kept and the browser can be
   opened again to download it again.
4. **A window waits for a browser's start as long as the start may take**
   (`A_START_TAKES_AT_MOST`: the manifest's read, the archive's download limit and a minute to
   start the engine), never the 30 seconds it gives any other answer. The person sees the
   download and may cancel it, so the long wait is never a silent one.

Decisions taken in implementation, to be confirmed:

- **D-2207-1** The download is shown in a Notice, one per download under way, wherever the
  person is, not on a browser's tab: a chat's browser has no tab while it starts, and the
  person's gets one only once it has started.
- **D-2207-2** The window names the browser it starts (a ULID in `StartProgram.program`), so it
  knows which start a pushed download belongs to, and drops a download's Notice once its start
  answers, whatever the answer.
- **D-2207-3** Progress is counted in MB, rounded down; a server that sends no length shows how
  much has come so far.
