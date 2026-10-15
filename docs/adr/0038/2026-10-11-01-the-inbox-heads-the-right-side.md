# Amendment, 2026-10-11: the Inbox heads the right side

The operator's decision of 2026-10-10 (spec #1688, I-2 and I-4; built in #1692): **the Inbox is
the right side's first view**, above Todos. It lists what waits on the person in the project, the
asks of the registry (#1690), grouped by chat with the chat waiting longest first, and answers
each in place through the path its source's Notice answers with. Its icon is the title bar's
hand, and its count is the asks', in the needs-you colours, on the bar while the side is away
too. Memory stays the view the side opens on.

**The title bar's ✋ opens the Inbox**, of the project in front where something waits there, and
otherwise of the first project along the strip with something waiting. The ✋ is no longer the
queue's only place: its count is the Inbox's. Its list no longer holds the dispatches refused
while nobody was there or the chats whose Smart close stopped: since #1693 each is an update in
the Inbox, answered there, and the away summary's part for the refused dispatches opens the
Inbox. #1695 retires what is left of the list. ⌘⇧I (Ctrl+Shift+I elsewhere) shows the
Inbox, and so does its palette row.

**Only an ask raises a system notification** (I-7, built in #1694): an update never does. One
chat's asks a few seconds apart share one notification, titled by its chain and saying the kind of
ask, never its words. None is sent while the window holding the project is focused with that
project in front and its Inbox open, or with the chat itself on screen. A click on one brings the
window forward, and the Inbox opens at that chat's group.
