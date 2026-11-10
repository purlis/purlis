### Added

- **Trust a clone you made outside purlis.** A workspace's clone that purlis did not clone, such
  as one made in your own terminal, stands as a notice in its project naming its folder and its
  remote. *Trust this clone* lets its saves push with your purlis sign-in and lets chats in that
  workspace use your GitHub access with it, at exactly the remote the notice showed.

### Changed

- **Saving a workspace's clone from the window no longer trusts it.** A Save of a clone purlis
  did not clone and you have not trusted pushes with your forge CLI's sign-in, as an automatic
  save does, and says to trust the clone where there is no CLI. Saving the project itself is
  unchanged.
