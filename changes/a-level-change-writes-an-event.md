### Fixed

- **A chat's Chat log says when its protection moved between Basic and Full.** A chat that started at the full sandbox and came back at the basic one, or the other way round, now writes one `trust.sandbox.level` event naming both levels and who made the start, and the Chat log shows it as "Protection changed from basic sandbox to full sandbox" (R-228, #6885).
