### Fixed

- The new-chat picker opens at once on a Mac with many chats running. Every tool call of every chat writes the project's state folder, and each write made the picker forget what it had read, so each open read it again under load. The state folder now leaves the picker's answer alone, and a write of the machine store that would change nothing is no longer made.
