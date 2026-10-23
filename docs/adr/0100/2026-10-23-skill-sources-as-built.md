# Skill sources, as #2773 built them

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-10-23**, ticket #2773 (SK-10). §5 said every fetch from a private source "runs through `purlis secret exec`" and that the token never reaches "purlis's own git runner, which strips credentials from what it starts". As built, the token **does** pass through purlis's git runner, by one variable. This file records how, and why that is safe. It also records the limits, the trust rule for committed files, and the failure rules the adversarial review of the ticket's pull request asked for.

## The token goes through `secret exec`'s steps into purlis's git runner

- The vault entry a source names is resolved by `secret exec`'s own code, `secrets::exec::prepare` and `record`, run in purlis's process. The vault is found as `purlis secret exec` finds it. A sandboxed chat is refused the keyring, as `secret exec` refuses it. The read is recorded by the vault's and the entry's names, never the value.
- The value is handed to **one** git process run by purlis's hardened runner (`worktree::git::run_network_handing`), as the variable `PURLIS_SKILL_SOURCE_TOKEN`. Beside it go `PURLIS_SKILL_SOURCE_USER` and `PURLIS_SKILL_SOURCE_HOST`. That git's configured credential helpers are reset, and the only helper it gets prints the user name and the token for `get` alone. It does so only where git asks for `protocol=https` and the source's own host. A redirect to another host, or a request over plain http, is answered nothing.
- **Why this and not a `purlis secret exec -- git` child.** A child of `secret exec` inherits purlis's environment. purlis's git runner builds the environment instead (`env_clear`, no hooks, no fsmonitor, the one-credential rule, ssh refused), and the token is then the only credential it holds. The token never reaches a command line, a lockfile, a settings file or the plane. Everything git says is redacted of it before purlis says any of it. What the runner strips by default stays stripped; this call adds one named variable and nothing else.
- A source that signs in must be an `https://` URL with a plain host (`savefix::https_host`: no user, no port). A source's URL never carries a sign-in.

## A sign-in, and contacting a committed source, are this machine's to say

- **A sign-in is read from `charter.local.toml` alone.** A committed `[[skill_sources]]` table's `vault` and `key` are not used. A teammate's push could otherwise pair the person's vault entry with a URL of its choosing, and the token would be handed to that host. Adding a shared private source writes the URL to `charter.toml` and the vault entry to `charter.local.toml`.
- **A source only the committed `skills.lock` names is not contacted** until this machine confirms it: Confirm in the studio, or `purlis skill update --confirm <url>`. The confirmation is a `[[skill_sources]]` table in `charter.local.toml`, and an install writes one. Until then, Updates lists the source as not checked on this machine. A folder on one machine that the committed lockfile names is never contacted from another, and is never offered a Confirm. A shared `[[skill_sources]]` table names an `https://` URL alone.

## Everything read from a source fails closed

- **Limits, each refused in one sentence:**
  - a fetch lands at most 256 MiB, and every blob over 1 MiB is left on the server (`--filter=blob:limit`);
  - a tree lists at most 50,000 entries, counted before any is read;
  - a source holds at most 200 skills;
  - a skill holds at most 1,000 files and 16 MiB;
  - one fetch's reads buffer at most 64 MiB;
  - a written path is at most 1,024 bytes.

  The scratch repository is a partial clone on the fetch's own command line alone, so a blob the filter left out is never fetched by a later read.
- **Names.** Every part of a written path is printable ASCII, not `.` or `..`, holds no separator, and does not start `.git` in any case. Two paths that case-fold together keep the first. Links and submodules are left out. All of these are listed in the review.
- **Formats.**
  - The listing is refused whole if one record is not git's.
  - A skill's frontmatter is read as YAML and offered only where every reader reads the same name and description.
  - The settings tables and the lockfiles are deserialised into closed shapes, and one bad entry refuses the file.
- **The review hides nothing.**
  - A file that is not text is listed as not shown.
  - A file is a script whatever its mode if anything runs it, or if it has a script's name or `#!` line.
  - Every Unicode tag, zero-width, bidi or other invisible format character is marked and shown escaped.
  - The command line escapes every control character it prints.
- **An install is all or nothing.** A skill, a grant or the lockfile that cannot be written takes back everything the install wrote, by exactly the names it made. An update writes the skills it brings along first, and takes them back where the update fails. Its own copy is replaced file by file, and the pin moves only once every file is written.
- **An update** shows the skills the newer commit refers to that the level lacks, in the same review, and installs them with it. It also shows the edits made here, as their own diff against the pin, beside the source's change. On a disk that ignores case, a file the newer commit only respelled is respelled and never removed.
