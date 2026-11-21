### Fixed

- **Changing the project's protection in Settings is now recorded.** Saving Settings › Project › Sandbox from Basic to Full (or any other change of level) writes one `trust.sandbox.level` event naming both levels and you, under no chat, so it can be read outside any chat's log. Saving the same level writes nothing. If the event log cannot take the record, the save tells you the change was not recorded (R-228, #6885).
