### Changed

- **Resolve settles a save's conflict in a folder beside the project.** The project itself is
  never left mid-merge: chats go on working in it, a quit leaves it as it was, and the resolve is
  there again when purlis starts. Continue moves the project in one step, and says which file is
  in the way if a chat changed one meanwhile. Abort removes the folder and changes nothing
  (#5824).
- **A setting both machines changed is chosen in the window.** Where both machines changed the
  same key of `charter.toml`, Resolve shows the key with both values and you choose one. No
  conflict marker is ever written into the file (#5852).
- **Saving after the remote moved replays this machine's commits without a rebase.** The
  project moves once the whole line is made, so nothing is left part-way (#5855).
