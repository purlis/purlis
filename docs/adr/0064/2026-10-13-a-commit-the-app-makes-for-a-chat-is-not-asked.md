# Amended 2026-10-13: a commit the app makes for a chat is not asked about (#1476)

Decided under delegation and flagged for the operator. `purlis worktree commit`, the commit the
app makes for a sandboxed chat in its branch folder (ADR 0067, amended for #1055), joins the
list on the dispatch's terms: `Bash(purlis worktree commit *)`, in
`harness::claude::WORKTREE_COMMIT_ALLOW`, an `allow` only, for Claude Code only, and an
operator's `ask` or `deny` still wins. The commit asks no approval of its own (ADR 0067's
decision 6): it is the chat's own work in its own folder on its own branch, as a chat's
`git commit` in a clone is, which no rule asks about either. A chat that is not sandboxed, or in
a clone, is told to use git, and nothing is committed.

It stands behind the rule for a rider (D-1444-14, `dispatchguard::rider_ask`): a call that runs
anything beside the commit, writes a file with a redirection on its line, or takes its message
from a command substitution is asked about as a whole. Every other `purlis worktree` word still
asks.
