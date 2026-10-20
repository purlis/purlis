# Amended for #2293, a typed event counts no keys, 2026-10-20

Two rows say `browser.typed` carries how many keys were typed: §13's event table ("how many
keystrokes (never the keys)") and the `browser.typed` row of the registry this record adds to ADR
0075 ("ADR 0075, amended": "the keystroke count"). A count of keys tells a typed password's
length. So in both rows, `browser.typed` names the browser and who typed (`person` or `purlis`),
and nothing else. It is written once for each stretch a driver types (ADR 0068, amended for
#2293). *Decided in implementation, to be confirmed (D-2293-4).*

Both rows also name the page's origin. `browser.typed` does not carry it yet, because the session
host does not know a page's origin when it writes the event. The origin of an event's page is
left to the ticket that writes the other browser events: `browser.opened`, `browser.closed`,
`browser.call`. *Decided in implementation, to be confirmed (D-2293-7).*

A browser's driver changing is `browser.driver_changed`, and a hand-over to a chat is
`browser.handed_over`. Neither carries a page's origin.
