### Fixed

- **`purlis workspace create <name> <repo>…` in a chat makes the workspace and its clones.** The
  app makes both in one ask, with the chat's internet access, as it makes a plain create and
  `purlis clone`. A sandboxed chat is told, before anything is made, that `--live` and `--use`
  are the person's to run. A workspace whose folder or baseline files could not be written is
  never reported ready: the command fails and names what is missing (qw387).

- **Refusing to remove a branch folder from a chat names the window's command:** *Remove folder
  <piece> in <repo>*, in the palette (qw387).
