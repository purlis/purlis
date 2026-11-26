### Changed

- A new chat that takes a second or longer to open now writes a line to the slow log
  (`slow.jsonl`) that names each phase of the opening with its milliseconds, such as waiting for
  the repo's turn, cutting the branch folder, readying the chat and opening its session, so a
  report of a slow new chat says where the time went.
