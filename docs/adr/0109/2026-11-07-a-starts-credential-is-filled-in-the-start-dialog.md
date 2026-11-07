# Amended 2026-11-07: a start's missing credential is filled in the Start dialog

#5074 made a chat start whose profile credential is missing raise the same Secret ask as a
brokered run, and refused the start, pointing to the ask in the Inbox (D-5074-2): the Start dialog
is modal, so a start waiting on the ask left the person no way to reach Fill. #5137 brings the ask
to the dialog instead.

Still, no start waits on its ask. The start is refused as before, and the core names the ask it
was refused on beside the refusal, as data: its vault, its key, the variable the chat is handed,
and the words for a declined start. The window never reads the refusal's words to find it. The
dialog draws that ask's own body, the one the Inbox draws, in place of the words that point to
the Inbox.

- **Fill** answers the one ask for that vault and key, as the Inbox would, so it closes there and
  in every list of the project's asks. The dialog then starts again with what it shows at that
  moment, and only while it is open. This happens only where ⏎ would start: approving a profile
  and starting without the sandbox each stay a press of their own (ADR 0097 §6), and the dialog
  says the key is filled.
- **Not now** declines the ask. Nothing starts, and the dialog says the core's words for a start
  whose credential was declined, with how to be asked again.
- A start's field offers no Change, because the start reads the key its profile names.

Any other start (a relaunch, a dispatch's) is refused and points to the Inbox, as before.
