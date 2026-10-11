### Security

- **The project-root guard reads a directory the same command makes or re-points.** A link,
  a move or a copy made earlier on the line, or a file a redirection writes there, is made
  before git runs, and the guard reads directories before the line does. Git sent to such a
  directory is now refused as one the guard cannot name, as a directory held in a variable is
  (#1326).

- **A program word only the shell can name is read for your rules.** A variable, a command
  substitution or a glob where the program stands may be a program one of your ask or deny
  rules names, or the command line under a consent rule. Where the words after it match such a
  rule, the command is refused, naming the spelling the rule matches (#1325).

- **A chat cannot run `purlis doctor --fix rename-plane`.** The fix already refused a process
  that carries a chat's session; the Bash guard now refuses the command itself, under every
  spelling it reads, so taking the session variable away does not get past it (#1288).

- **The handoff guard reads a string that spans lines in a code span GNU bash 3.2 runs.** The
  consent and rule backstops already refused it (#1426).

- **The release floor's reading of one command is bounded.** It read each level of a nested
  command whole, so a long command nested deep was read once per level. Its whole reading is
  now held to a few times the command's length, and a command past that is refused as one the
  floor cannot read (#1409).

### Changed

- **A command fed to a shell on its stdin, or run by `find -exec`, is told so.** A refusal of
  one used the sentence for a string or a heredoc a shell runs; it now names where the command
  was fed (#1426).
