# Amended for #2013, a chat's browser tools, 2026-10-14

A chat's agent drives a real browser through typed tools, the same on every harness, carried on
the hook socket's framed stream; the first browser in a chat asks the person once what it may
reach (this project's sites and its dev servers, also the open web, or off); a screenshot is a
binary result; and a sandboxed chat no longer offers its harness's own browser tools. That is
the operator's ruling of 2026-10-14 (spec #2008's grill, "agree at all"). The amendments above
are left as accepted, and this note adds to them. Items 1 to 6 are **decided in
implementation, to be confirmed** (D-2013-1 to D-2013-6).

1. **One vocabulary for a browser's acts, in the session protocol.** `browsing::Act` (open,
   navigate, snapshot, find, click, double-click, hover, type, press, select, drag, scroll,
   back, wait for text, pages, switch page, screenshot) and `browsing::Browsed`, its answer, are
   what the chat's tool server asks the window over the hook socket and what the window asks the
   session host. Only the driver itself turns an act into its own call. *D-2013-1, to be
   confirmed.*
2. **Session protocol 1.9, a minor.** `browse` carries one act to a browser under the act's own
   id, answered `browsed`, refused `browser_refused` with the driver's sentence, or refused
   `browser_ended` where the browser has ended. A browser's start may carry `reach`: the hosts
   its chat's sandbox lists, each at its layer, the hosts policy pins as never allowed, the
   ports of this machine withdrawn from the chat while it runs, and `open_web`; absent, nothing
   is listed and every site new to the project is refused, as before. `browse` is `local-ui`'s alone (FD-27's `browser` row),
   and its acts keep their order per browser in a lane of their own, apart from the browser's
   views. Recorded in the protocol's fixture. *D-2013-2, to be confirmed.*
3. **The window decides; the host enforces.** The window answers a chat's browser call: it asks
   the person the first time (one Ask in the asks registry, "This project's sites", "Also the
   open web", "Keep off"; the chat's later calls ask nothing more), starts the chat's browser in
   its session host reaching what the chat's sandbox lists (never its floor), and hands each act
   on. The host's browser carries it out, and its reach proxy holds what it may reach. The
   answer reaches the chat in the one line of every Ask. A project's chats hold at most three
   browsers together (spec #2008's Limits), each named to its chat `b1`, `b2`, looked up among
   its own alone, and they end with it. A browser's reach follows its chat's reach, including
   what the person withdraws: when the person narrows a chat's reach while it runs, its
   browsers end and its tab says so, and its next call opens one on what the chat reaches then.
   Held in memory: a relaunch asks again. *D-2013-3, D-2013-7, D-2013-11, to be confirmed.*
4. **A screenshot fits the control lane.** A browser's answer crosses the session protocol, whose
   control frames are bounded, so a screenshot's PNG past 600 KiB is halved until it fits, and
   the answer says so. On the hook socket it is attached as `image/png` after the answer, and the
   MCP server hands it to the model as an image. *D-2013-4, to be confirmed.*
5. **Harness-native browser tools.** A Claude Code chat purlis sandboxes, at either level, is
   denied its Chrome integration's tools. Codex and opencode offer no browser tool of their own
   today. *D-2013-5, to be confirmed.*
6. **The harness's own prompt is unchanged.** A browser tool is asked about by a harness that
   asks about MCP tools, as any purlis tool that changes something is; whether it should run
   without that prompt, its consent being the person's answer to the browser's Ask, would amend
   ADR 0064 and waits on the operator. *D-2013-6, to be confirmed.*
