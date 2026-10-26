# Amended for #2017, the app keeps a chat's browser timeline, 2026-10-26

Spec #2008 lists keeping the timeline among the broker's jobs, beside the driver lease, reach,
logins and frames. **The app keeps it instead, not the session host** (D-2017-1, accepted in the
review of #3286). The broker still holds the lease that decides when the agent may act; the
timeline only records what came back.

1. **A chat's timeline is the app's record of the answers its browser calls were given**:
   each act of the agent's in words, the page it left, and the screenshot the agent was
   answered, already blanked by the browser. So it never holds more than the model was given.
   It takes no screenshot of its own.
2. **"Kept on this device" means the person's device.** Once a browser runs on a runner
   (spec 3), its session host is not on this device, but the app always is.
3. **It belongs to a chat, by the chat's id** (ADR 0066), which only the app knows. It is
   removed when the chat closes, and swept each time the app writes its record of open
   chats: a chat not in it loses its timeline, though one written in the last two minutes is
   spared, since a chat that has just started may not be in the record yet.
4. **No session-protocol command reads it.** It is the app's own store
   (`app/browser-timelines/<chat>/`, `docs/plane-format.md`), which the window reads through
   window-only commands and no chat reads at all.
