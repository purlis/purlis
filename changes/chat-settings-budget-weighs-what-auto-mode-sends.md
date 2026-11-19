### Fixed

- **A chat's settings budget now weighs what auto mode sends, and a chat whose settings are too big says so.** The budget counted the rules as written; Claude Code's auto mode sends each one with more beside it, and the person's, the project's and the managed policy's rules too, so the tab's notice could come after auto mode was already off. The budget now weighs each rule as it is sent, with those files' rules. A chat whose whole settings file is past 768 KiB is not started, with a reason, rather than failing silently in the sandbox. A folder name with a `]` in it is held as the pattern Claude Code reads it as (#7157).

- **The notice says what is shed, in two steps.** Past 128,000 bytes of rules Claude Code's auto mode may drop your allow rules; past 256,000 it also sheds the git state and history it checks with. A settings file over 1 MiB that a chat reads is counted as over the budget, not as nothing (#7225).
