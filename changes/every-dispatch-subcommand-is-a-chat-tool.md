### Added

- **Every `purlis dispatch` subcommand is a chat tool.** `dispatch_wait`, `dispatch_tell`,
  `dispatch_answer`, `dispatch_cancel`, `dispatch_close`, `dispatch_note` and `dispatch_ask`
  join `dispatch`, `dispatch_list`, `dispatch_read` and `dispatch_report`, on every harness. A
  Claude Code chat runs each one without its harness asking. Chats are taught the tools first,
  and the command stays for terminals and scripts (#3786).
