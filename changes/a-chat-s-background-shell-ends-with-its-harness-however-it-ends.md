### Fixed

- A background shell or monitor a Claude Code chat started now ends with the chat's harness when the harness exits on its own or is ended from outside purlis, not only when purlis ends it. Before, such a shell kept running on its own, with nothing in purlis that could still see it (#5806).
