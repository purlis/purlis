### Changed

- **A rule's mode, its family's mode and a lock now decide together as the policy spec says (M75-10, #7019).** A rule's own `[policy.rules]` mode beats its family's, a lock beats both, and where several rules decide, each runs at its own mode (or its family's) and the strictest wins, so one rule set to `log` no longer loosens a forbid beside it. A lock on `asks` binds an ask on `read`, `write`, `connect` and `dial`. `log` on a permit or on a `vaults` rule reads as `enforce`, with a Notice. `audit` is a family of its own and reads `off` or `log` as `enforce`, with a Notice.
