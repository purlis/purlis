### Changed

- **A chat does not start when purlis cannot tell what to keep from it.** In a project that runs
  its chats sandboxed, a vault registry that does not read, or a plain-file vault whose file is
  said in a way purlis cannot use, refuses the start for every harness, with a sentence that
  says why, gives the path of the file to put right, and names Doctor. A project with no vault
  registry, with vaults in the keyring, or with a vault this machine has not set up yet starts
  as before, and a chat that is already running is not stopped.

- **`purlis doctor` says what it makes of the vaults it keeps from chats.** A
  `sandbox kept files` row says why the list is not whole, when it is not, and otherwise names
  an entry left over on this machine, a vault of a kind this purlis does not know, and a vault
  not set up on this machine yet. The vault's view says the same of each.

- **A workspace's files leave out what the project keeps to itself.** A vault's file inside a
  workspace's repo is not listed, opened, searched, found by name or compared on that
  workspace's tab, as it is not among the project's own files.
