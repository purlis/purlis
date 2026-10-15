# Amended 2026-09-25: commands on the command line (purlis#342)

- **`cli`** adds commands to the `purlis` command line (`contributes.cli`: `name`, `title`,
  and `writes`, which is required). They run as `purlis <extension id> <name> <args…>`. A
  command that writes needs the `writes` capability's declared paths, and the approval prompt
  lists it; one that only reads is not listed, beyond the capability's own line.
- **It is protocol 2, not 3.** A command is one more request kind, *run command `<name>` with
  `<args>`* (`command` and `args`), grown into protocol 2 beside #343's events and briefing
  while 2 was unreleased. Its answer is not a line of JSON: the program's stdout and stderr and
  its exit status are passed back to the caller unchanged, each bounded at 512 KiB and read to
  the end within the normal deadline. A program that is still running at the deadline, is
  killed by a signal, or prints more is a refusal of purlis's own, and none of its output is
  passed on. **The deadline is the executor's 5 seconds, unchanged.** "Process life is
  unchanged" (purlis#336). A command that needs longer, streams its output or reads the
  caller's stdin is a change to process life, and it is its own decision, not a capability's.
- **The naming rule above is enforced where the id is read.** An id that is one of purlis's
  own command words refuses the whole manifest, for every extension, so at install, at
  approval and at every later read. The list is `extension::cli::CORE_WORDS`. The core cannot
  read the `purlis` binary's parser, because the app approves extensions and does not link the
  command line, so a test in the binary reads every word off the parser and fails for one the
  list is missing. The binary also asks its parser first, so a core word never reaches an
  extension whatever a record says.
- **A core-owned alias is a row in `extension::cli::aliases()`.** It has the core words it is
  typed as, the extension and the command. It is in the core so that the tool guard reads an
  aliased call as the extension command it runs. None exists in a release build; a test build
  has two onto the probe, and a test proves an alias's output is byte for byte the direct
  command's. `purlis ws todo` has not moved.
