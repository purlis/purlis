### Added

- A person can allow a chat to run one command with a vault's keys, even where the program is no declared tool. The chat's `secret exec` raises a notice that shows the program's full path, the keys, the hosts and the command as plain text; Allow once runs that exact command in the chat's own sandbox, and Don't allow is not asked again about that program and vault. Programs that run what they're told, and `purlis` itself, stay refused (ADR 0111).
