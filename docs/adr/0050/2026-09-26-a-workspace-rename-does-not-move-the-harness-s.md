# Amendment, 2026-09-26: a workspace rename does not move the harness's conversations

The operator's ruling D10 on charter#367. Claude Code files each conversation under
`~/.claude/projects/<encoded cwd>/` and finds it again only from that folder, so a chat reopened
after `purlis workspace rename` cannot resume it. **purlis does not move that folder**: it is
the harness's own file, and the rule above ("never write the harness's config") covers it.

Instead the rename goes ahead and first names each chat that will start a fresh conversation:
the terminal prints it, and the window's Rename dialog shows it before it is answered. The app's
record then drops those chats' conversations, and reopening one starts fresh with a note saying
why, once. Which harnesses lose a conversation is an adapter fact
(`Harness::keeps_conversations_by_directory`). Only Claude Code does. Codex (`codex resume
<id>`) and opencode (`-s <id>`, a session keyed by the repository and not by a path) find a
conversation by its id from any folder, so they are not named and are not reset.
