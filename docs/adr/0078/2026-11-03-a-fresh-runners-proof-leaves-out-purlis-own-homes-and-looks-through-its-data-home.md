# Amended 2026-11-03: a fresh runner's proof leaves out purlis's own homes, looks through its data home, and looks through a large image once

**Built by #5028, #5029 and #5030**, following the amendment of the same day by #4933 (a fresh
runner's proof survives a host restart and looks beyond its home). What follows is *decided in
implementation, to be confirmed* (the D-lines of the pull request that built it).

1. **purlis's own homes are left out wherever the environment puts them** (#5028): its config
   home's folder (`$PURLIS_CONFIG_HOME` or `$XDG_CONFIG_HOME`, and the session host's default),
   the lock it holds beside it, its data home (as `$XDG_DATA_HOME` places it, or under the home;
   one `$PURLIS_DATA_HOME` names is the data home in use, looked through by point 2), the app's
   own folder under the data folder (its logs) and its socket's folder in `$XDG_RUNTIME_DIR`,
   each under every name it has had. Only folders purlis names are: never one a variable names
   as it is, which could be a folder others share (`$PURLIS_LOG_DIR=/srv`), so a log folder
   that variable moves counts (`purlis serve` logs to its standard error); never the home or a
   folder holding it; and never a state or cache home moved outside the home, which holds other
   programs' files. purlis keeps nothing of its own in either.
2. **The data home is looked through** (#5029), by rules of its own: a file a task saved there
   outside every repository keeps the runner (beside the projects, loose in the projects folder,
   or in a project's folder with no repository). Left out: each store purlis keeps there itself,
   by name at the data home's top (the plane format's stores, and the homes it makes for
   chats), the mark of the first program and a transfer's scratch (`.code-<random>`) beside the
   projects, and each repository listed on its own. A repository is listed only where git takes
   its folder for the repository it reads; a `.git` it does not is said, which keeps the runner.
   A home inside the data home is looked through by the home's rules. A test holds the names to
   the plane format, since a store missing from them would keep every fresh runner.
3. **A runner's file system beyond its home is looked through once, for where its user may
   write** (#5030): a folder it owns or may write in now is a place, and each check walks only
   the places and stats each file the user owns outside them. Neither set can grow: a runner's
   user is never root, holds no capability and gains none (every capability dropped,
   `no_new_privileges`, in a pod and a Docker pool alike), so it can neither change the mode
   or owner of a folder it does not own nor mount one. The look is kept only while it read
   everything, and made again for a new cutoff, for what else is left out, and when the host
   starts again. It records at most 100,000 files of the user's own outside the places; past
   that, it is said. An image's own folders count toward that one look, bounded at
   2,000,000 folders, and never toward a check's 200,000; what the look can't read is said again
   at each check, which keeps the runner as before.
4. **Rejected for #5030:** leaving out an image's own files known from its layers (the image's
   layers are not readable from inside the pod, and a file the user owns in a layer may still be
   changed); walking only the mounts the pod declares (a team image may give its user any folder
   on the root file system); and a larger bound for every check (each check would still walk the
   whole image).
