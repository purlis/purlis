### Added

- **Install skills from a git repository in the Skill studio preview** (#2773).
  - **Add source** takes a repository's https:// URL, or a folder on this machine, and optionally its branch and the folder its skills are in. Browse lists the skills it holds.
  - **The install review leads with a summary**: the scripts a skill carries and what runs each, the network hosts its files name, its `!` commands, anything it writes outside the workspace, any text hidden from a reader, and any file that is not text. Risky items are marked, every file is one click away with hidden characters shown, and skills it refers to come along in the same review.
  - **An install is a copy, pinned** to the commit you reviewed, all or nothing, and one commit of the skill's files and `skills.lock`. A local install is kept in `skills.local.lock` on this machine.
  - **Updates are never applied by themselves.** A newer commit is offered with what its summary gains and loses, then each file's diff, your own edits beside it, and any skill it now needs; only Accept moves the pin. A source only the team's record names is not contacted from your machine until you confirm it.
  - **A private repository signs in with a vault entry** of your own: the token stays in the vault, is sent to that repository's host alone, and is never shown or written anywhere.
  - The same on the command line: `purlis skill install` prints the review and asks before it copies anything, and `purlis skill update` lists newer commits and shows each one's diff before it asks; `purlis skill update --confirm <url>` confirms a source.
