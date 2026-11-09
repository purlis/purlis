# Amended 2026-11-09: a Run grant refuses a flag that names a command

Decided under the operator's standing order to take the recommended path (D-5799-1 to D-5799-16,
#5799, from the adversarial reviews of #5798 and #5876), recorded here as a delegated decision.

`ssh`, `scp`, `sftp`, `rsync`, `kubectl`, `psql`, `mysql`, `mariadb`, `zip` and `wget` stay
offered Allow once on the exact command (D-5619-7): they are the usual reason a chat is handed a
key. A command of theirs whose flags name a command, or a file or library that can run one, is
refused before anyone is asked, and the chat is told to leave the flag out. A granted run has no
stdin, a home of its own and a `PATH` without the folders a chat writes, so what else it reads is
in the command the person reads.

- **Read as their parsers read them, and wider where in doubt:** short flags in a cluster, a
  value in its word or the next, a long option by any abbreviation, every word up to a `--`,
  and, for `rsync`, whose operands' hosts reach ssh as words of their own, the words after it.
  Each row names the getopt string its letters come from: a letter wrongly taken to have a value
  would hide the word after it.
- **A value ssh pastes into a command its shell runs** (`-J`, and `-o ProxyJump`, `Hostname` and
  `User`) is refused when it holds anything but what hosts, users, ports and jump lists are
  spelled with, or a user or host starting with `-`. For `rsync`, an operand whose user or host
  starts with `-` is refused wherever it stands, since rsync hands each to ssh as a word of its
  own. A host in IPv6 brackets is judged with them taken off; an
  operand's host is read as OpenSSH's `colon()` reads it (a `[` opens brackets only at the start
  or after `@`, and a `/` before the first `:` makes it a local path); brackets are let through
  only as a whole `[address]`; and a `scp://`, `sftp://` or `ssh://` URI's host is its authority. Where a jump is used, the
  hosts ssh pastes into the jump's command (ssh's and sftp's destination, scp's remote hosts) are
  held to the same rule.
- **A program's own tools are read by rows of their own**, ahead of the row their name's first
  part would reach: `ssh-keygen` and `ssh-add` (their libraries), and mysql's and mariadb's
  tools, every name but the client's (their files of settings and plugins), whose row takes no
  letter to have a value. Their server-side and shell-like tools (`mysql.server`, `mysqld_safe`,
  the install scripts, `mysqlslap`, `mysqltest`, `mysqlsh`) are refused whole.
- **By every name a build goes by** (`kubectl.docker`, `psql-16`, `wget2`), as names refused
  whole are.
- **A program that is the language of what it runs, or reads a file a chat wrote, is refused
  whole** (`sed`, `aria2c`, `kustomize`), and so is one that runs the command after it or hands
  ssh whatever it is given (`sshpass`, `autossh`, `mosh`, `lftp`).

A form caught by chance takes away an Allow and nothing more (D-5424-1). A person can still
declare any of these as a tool. The table is in `crates/purlis-core/src/sandbox/tools/command_flags.rs`.
