### Security

- **More of the code a project's hooks load is kept from a sandboxed chat's writes.** A bare
  name Node, tsx or bun is asked to load first (`-r`, `--import`, `--preload`) is now also
  protected in each `node_modules` from the folder the hook runs in up to the project, with the
  file a package's `main` names. Ruby's `RUBYLIB` is followed as its `-I` is, and Perl's `-M`
  and `-m` modules are protected where Perl looks them up, along `PERL5LIB` and `-I` (#1425).

### Changed

- **A chat refused macOS's per-user temporary folder is told where to write instead.** No Allow
  is ever offered for those folders, so the chat is no longer told to wait for one: it is told,
  in fixed words, to write under its own `$TMPDIR` (`mktemp -p "$TMPDIR"`). Any other block in
  the same turn is still waited on (#1416).
