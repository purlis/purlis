# Amended (2026-11-03): an offer is carried by an event's answer (#4769)

The [2026-11-02 amendment](2026-11-02-a-pane-holds-a-chat-a-view-or-nothing.md) ruled that,
without a press, an extension can only offer its view, with one quiet note, and that the
manifest gains no key. It did not say how an offer reaches purlis. Today an extension speaks only
when it is asked, and nothing it answers to an event was read.

**An offer is carried by the answer to an event the extension hears.** The answer may carry one
`offer`: `view`, one of the extension's own declared views, with an optional `key` (what it is
about) and an optional `chat` (which chat it is about). The core refuses an offer of any other
view, as it refuses a row offering an undeclared action, and refuses an offer with any other word
in it. So an extension can never offer another extension's view or one of purlis's, and never
says where a view goes (property 3 holds).

The window shows an offer as the 2026-11-02 amendment ruled. An offer never opens anything. A
second offer while one stands, or while the view is open, is dropped. Only an event the window
told is shown: the command line and a chat's start have no window to show one in.

Ruled by the orchestrator of milestone M69 on 2026-11-03, as a delegated decision on #4769:
the spec ([#4750](https://github.com/purlis/purlis/issues/4750), story 103 and its Extension
views rules) already says an extension offers its view and the manifest gains no key, and an
answer-carried `offer` meets both. No new capability: `events` is the grant it rides on.
