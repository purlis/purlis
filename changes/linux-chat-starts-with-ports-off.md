### Fixed
- On Linux, a chat in a project whose `[policy]` turns `ports` off (the `open` template, for one) now starts. Its sandbox refused the profile purlis wrote for it, a deny of purlis's own sockets by name, and started nothing; those sockets stay denied.
