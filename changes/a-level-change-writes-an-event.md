### Fixed

- **Changing the project's protection in Settings is now recorded.** Any change of the project's level (a save in Settings › Project › Sandbox, a raw edit of either settings file, or a Confirm) writes one `trust.sandbox.level` event naming both levels and you, under no chat, so it can be read outside any chat's log. Saving the same level writes nothing. If the event log cannot take the record, the level is still saved and Settings tells you the change was not recorded (R-228, #6885).
