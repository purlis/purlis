### Added

- **Install skills from a git repository in the Skill studio preview** (#2773).
  - **Add source** takes a repository's https:// URL, or a folder on this machine, and optionally its branch and the folder its skills are in. Browse lists the skills it holds.
  - **The install review leads with a summary**: the scripts a skill carries and what runs each, the network hosts its files name, its `!` commands, and anything it writes outside the workspace. Risky items are marked, and every file is one click away. Skills it refers to come along, in the same review.
  - **An install is a copy, pinned** to the commit you reviewed, and is one commit of the skill's files and `skills.lock`. A local install is kept in `skills.local.lock` on this machine.
  - **Updates are never applied by themselves.** A newer commit is offered with what its summary gains and loses, then each file's diff, and only Accept moves the pin. A copy you edited says so.
  - **A private repository signs in with a vault entry**: the token stays in the vault and is never shown or written anywhere.
  - The same on the command line: `purlis skill install` prints the review and asks before it copies anything, and `purlis skill update` lists newer commits and shows each one's diff before it asks.
