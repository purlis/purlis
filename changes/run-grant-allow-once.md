### Added

- **Allow one program with a vault's keys.** Where a chat's `secret exec` names a program its persona doesn't declare, a notice shows the program's full path, the keys, the hosts, the folder and the command as plain text. Allow once runs that exact command in the chat's own sandbox, with no input from the chat, and Don't allow is not asked again about that program and vault. A vault the persona can't otherwise use opens for that alone, with nothing kept. Programs that run what they're told, and `purlis` itself, stay refused (ADR 0112).
