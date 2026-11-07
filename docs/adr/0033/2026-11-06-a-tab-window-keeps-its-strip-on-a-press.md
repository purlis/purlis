# Amended (2026-11-06): a tab window keeps its strip on a press (#5330)

Records [A press from another workspace can flip a tab window's strip](https://github.com/purlis/purlis/issues/5330),
which says the rule: a press from another strip doesn't change which strip a tab window shows. It
narrows "the most recently used pane across the project's windows" in
[the amendment of 2026-11-02](2026-11-02-a-project-may-have-tab-windows.md), and leaves the
opening rule ([ADR 0038's amendment of 2026-11-05](../0038/2026-11-05-a-press-lands-by-the-opening-rule.md))
as it is.

**A press on a chat in no pane, filed on a strip the tab window isn't showing, opens in the project
window.** The tab window that was used last changes nothing, and the project window comes forward
and opens the chat by the opening rule on that chat's strip, and its pane counts as the one used
last. A tab window holds only the tabs torn
off into it, so taking the chat would mean a new tab there and a switch to another strip. The
project window holds every strip and already changes strip on a press. A chat on screen in a tab
window is still gone to there, whatever its strip, because a chat never shows twice.

A new tab window was the other choice. It isn't taken because a press never makes a window: only
Move to new window and a tear-off do.
