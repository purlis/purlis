# Amended 2026-11-08: a forge program's folders are held where a variable moves them

Ruled by the person on 2026-11-08. The amendment of 2026-11-02 let a person lift `~/.config/gh`,
`~/.config/glab-cli` or `~/.local/share/gh` for one chat at the floor; those names were held only
at their usual place in the home, so a machine whose environment moves them left the moved folder
writable.

1. **gh's and glab's folders are held where the person's environment puts them, on every
   machine.** No chat writes `$GH_CONFIG_DIR`, `$GLAB_CONFIG_DIR`, `$XDG_CONFIG_HOME/gh`,
   `$XDG_CONFIG_HOME/glab-cli` or `$XDG_DATA_HOME/gh`, or anything in them: each holds settings,
   aliases or extensions the program's next run loads, outside any sandbox, as the folders at
   their usual place do. Each variable is read whether or not another one wins, because which
   folder a later run reads is that run's environment's to say.
2. **Only an absolute folder counts.** A variable that is empty names nothing, and one that names
   a relative path names a different folder for every folder a run starts in, so no one folder.
3. **A variable never takes the chat's own work from it.** A folder that is the home, a folder
   above it, or a folder that holds the project is not held for this reason.
4. **A moved folder is not one a person can lift.** A lift names a folder below the home by its
   usual name. A chat that has to write gh's settings where a variable moved them runs in your
   own terminal.
5. **A runner is unchanged.** It already keeps these folders from every chat, read and write.
