### Added

- **Copy diagnostics names the device's RAM class and its hot-chat target.** The report's chats line
  now reads, for example, `Chats: 12 open, 4 hot, target 15 (16 GB class)`: the class is the largest
  of 8, 16, 32 and 64 GB your memory reaches (under 8 GB targets one), and the target is a budget
  that warns and never refuses a chat (SC-4f, #7635, part of #1391).
