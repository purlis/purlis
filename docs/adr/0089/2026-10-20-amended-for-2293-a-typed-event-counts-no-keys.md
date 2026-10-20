# Amended for #2293, a typed event counts no keys, 2026-10-20

§13's `browser.typed` row said the event carries how many keystrokes were typed. A count of keys
tells a typed password's length, so `browser.typed` names the browser and who typed (`person`
or `purlis`) and nothing else, written once for each stretch a driver types (ADR 0068, amended
for #2293). A browser's driver changing is `browser.driver_changed`, and a hand-over to a chat is
`browser.handed_over`; neither carries a page's origin. *Decided in implementation, to be
confirmed (D-2293-4).*
