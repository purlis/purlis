### Added

- **Move a chat to a pool's runner and back.** A chat's tab offers *Move chat … to pool …* and,
  on a runner, *Move chat … here*. The move is prepared while the chat keeps working and happens
  at its next pause between turns, never mid-turn: its branch, uncommitted work and conversation
  go with it, and its workspace's chats run there from then on. A move that fails leaves the chat
  where it was and says why. Claude Code chats move today (#2048).
