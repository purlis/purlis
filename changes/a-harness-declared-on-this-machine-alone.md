### Added

- **A harness can be declared on this machine alone.** A `[harnesses.<name>]` table in
  `charter.local.toml` declares a harness in the same form as a project's
  `harnesses/<name>.toml`. Its chat runs in its terminal, and it is asked about once, as
  every program that file names is: the question shows the words its declaration adds, and is
  asked again when they change. Under the name of a harness purlis ships (`claude`,
  `codex`, `opencode`) the table replaces that harness's measured facts on this machine: its
  terminal and its card. It never changes how the harness is run or armed: the program, the
  levels, the words purlis starts and resumes a session with, the environment namespace, the
  adapter and the sandbox stay purlis's, and a table that says otherwise is refused with its
  reason. The tables are read only where git would not carry the file. A prompt is typed into a replaced harness on one line only (#968).
- **A chat on a declared harness has its card's button in its header**, as a chat on a harness
  purlis ships has (#1134).

### Fixed

- **Open file on a harness declaration works with a terminal editor.** Settings opens the
  declaration in a shell tab at the project root when `$VISUAL` or `$EDITOR` is a terminal
  editor, as a branch's file opens (#1044).
