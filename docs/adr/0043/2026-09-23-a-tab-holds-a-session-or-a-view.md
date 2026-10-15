# Amendment, 2026-09-23, later the same day: a tab holds a session or a view

**This amendment is not a sign-off either, and the DRAFT line at the top of this record still
stands.** It records a ruling the operator did make, and the design built on it that he has not
yet ruled on. The work is purlis#212, unmerged when this was written.

**The sheet above is withdrawn.** The previous amendment called the non-modal sheet *"a decision
made without the operator … the one in this amendment most worth his attack"*, and it was put to
him: where should a persona's card — its memories and the Statistics button — open? Offered the
sheet, a popover, and a tab of its own, **he chose "Its own tab"**, and said why in words that
widen the question past personas:

> *"we dont have other tabs then sessions, and this can be good example for us - that in tabs we
> can have what we want - not only harnesses, so lets have this, and we can for future use tabs
> concept for lot of things.."*

His goal is the *"100% pluggable app"* this record opened with: the todos and the personas are
meant to be pure examples of plugins, and what the core does for them any extension must be able
to do. So what is recorded here is not *the persona card moved*; it is what a tab is.

**1. A tab is a layout of panes, and a pane holds a session or a view.** The pane is what is
generalised, not the tab. A tab stays one thing — a layout — so a view can be split beside a
chat, and every rule a tab already had holds for a tab that shows no chat without a second copy
of it: the fixed order (ADR 0039), pinning, the overflow menu and its `N more`, keyboard reach
through the palette's rows, and splitting and closing the pane in focus. The chat's name lives
on the session a pane shows, so a tab holding only a view has no chat to pretend about — no state
mark, nothing for "the chat in front" to mean, and a close that ends nothing and asks nothing.

**2. A view is named by data, and purlis's own views and an extension's take the same path.**
A view is *who draws it, which of theirs, and what it is about*: `{ from: null, view: "persona",
key: "steward" }` for purlis's persona view, `{ from: "persona-statistics", view: "statistics",
key: "" }` for an extension's. One command answers both (`open_view`), in this record's
vocabulary and nothing else, and one renderer draws both. **purlis's persona view is produced
as `panel::Block`s in Rust** — the definition as notes, the memory count, the memories as a
list — rather than as a component that knows what a persona is; the window cannot tell it from
an extension's view except by whose it is, which it says. That is the operator's test, applied
to purlis's own code: nothing the core does for the persona view is a door an extension's view
is refused.

**3. Views are the extension point for tabs.** An approved extension that offers a view gets a
tab when the operator opens it — from a panel's heading, from a view about the same subject
(the Statistics button on a persona's tab), or from the palette — and nothing it declares says
*where*: property 3, carried to a surface the size of the window. A view tab opens on the strip
in front, carries that workspace (it works in no directory to be filed by), and opening the same
view again brings its tab forward rather than drawing a second.

**4. What a view may do at a launch is less than what it may do at a press.** View tabs are in
the plane's reopen record beside the chats, so they come back; the record is writable by
whoever can write the plane's state directory, so **a tab the record put back asks an
extension's program nothing until the operator presses for it**. Opening a persona's tab asks no
extension anything — the Statistics button is the operator's press, and it opens its own tab.
A view whose source has gone (an uninstalled extension, a deleted persona) comes back as a tab
that says so, not as an error. Views start nothing, so they are not part of the trust
fingerprint (ADR 0035).

**What this record does not yet say, and should before a second view kind lands:** whether an
extension may offer a view that is about nothing purlis publishes (today every extension view is
about a `Subject`, and `personas` is the only one); whether a view may be split beside a chat by
anything but starting a chat beside it; and what a view that wants to change something — a verb
— would need, which is the `runs` grant this record already describes and nobody has asked for.
