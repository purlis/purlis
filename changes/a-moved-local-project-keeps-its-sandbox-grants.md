### Fixed

- **A local project moved out of the config home keeps its sandbox grants.** The move now
  points each folder you let chats write, each folder you listed as one chats may be granted,
  and their records in the Granted list at the same folder in the new place. Only a folder in
  the project is pointed, step for step, and never one reached through a link there: such a
  grant is left as it was, and the link at the old place is kept. While that link is kept, the
  doctor's `local project` row says why in the window, not only in the app's log (#1698).
