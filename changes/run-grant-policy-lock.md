### Added

- A new policy lock, `sandbox.run-grants`, turns Run grants off, and is allowed by default. Under it, or under the `persona-tools` lock, no Allow is offered and the chat hears why and to declare the program as a tool; a key the command misses can still be filled. A chat on a runner gets no Run grant and hears the same. A refusal under policy is audited.
