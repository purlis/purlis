# A skill is installed from a git source as a pinned copy, and never updates by itself

**Accepted 2026-10-20**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20 (decisions 7, 20, 29, 39, 44 and 57); ticket #2773
(SK-10). It builds on [ADR 0099](0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md):
an installed skill is an ordinary skill at a level. It leaves
[ADR 0041](0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md)'s "no registry, no fetch by
name" standing for extensions, and says why skills are different.

**Built by #2773** (sources, install, review, pin, update, private sources), then #2774 (org
controls) and #2781 (publish and export).

## Where purlis was

A person could not install a skill at all. The public collections of skills are git
repositories of SKILL.md folders, and the only way to use one was to copy its folders by hand
into a harness folder or a clone, where purlis could not see them, nothing said which commit they
came from, and nothing said when the source changed. A skill can carry scripts and `` !`cmd` ``
lines that run in the chat, and nobody was shown them before they ran.

## The decision

### 1. A skill source is a git repository

A **skill source** is a git repository of skill folders: its URL, and optionally a branch and a
folder inside it. A person adds one in the Skill studio or with `purlis skill install`, and
browses its skills there. The first open of the studio offers the first-party catalogue and
well-known sources with one click (#2785); every other source is one the person adds by its URL.
purlis resolves no name to a URL and runs no index of sources.

The sources a project browses are a project setting, shared or local as any setting is. A
source records its URL and, for a private one, the name of the vault entry it signs in with
(§5), never a credential.

### 2. Installing copies the skill into a level, pinned by commit

**An install copies the skill's folder into the chosen level** (project, workspace, persona, or
the local form of each, ADR 0099 §1) and records it in the project's **`skills.lock`**, at the
plane root: the skill's level and name, the source's URL and folder, the **commit** it was copied
from, and a digest of the copied tree. A local install is recorded in `skills.local.lock` beside
it, which is never committed, under the same live/local rule as `charter.local.toml`
(ADR 0048). Both are **Plane**-tier stores (ADR 0069): the shared one is committed, the local one
is clone state.

- **A copy, not a link or a submodule.** The bytes a chat runs are the bytes the person
  reviewed, they sit in the plane's history beside every other change, and the skill works with
  the network off. A submodule would put a second repository in every clone, and a link would
  run whatever the source holds today.
- **Skills a skill refers to come along.** The references in its files (the uses/used-by graph,
  #2782) are followed in the source, and each one brought is installed and locked as well,
  marked with the skill that brought it, so an installed skill works on its first run. A
  reference the source does not hold is a lint finding, not a failed install.
- **The grant is asked for as any project skill's is.** An install at the project level lands
  with the grant `root` unless the person picks another in the review (ADR 0099 §2).
- **The install is one commit** through purlis's normal save: the copied folders and the
  lockfile entry together.

### 3. The review leads with a summary

Before anything is copied, the studio shows the **install review**. It leads with a summary of
what the skill does beyond its instructions:

- the scripts it carries, and what each runs;
- the network hosts its files name;
- its `` !`cmd` `` lines, which some harnesses run before the model sees the skill;
- writes it would make outside the workspace;
- its grade, if the source carries one (#2781), and the lint's findings (#2771).

The risky items are marked, and every file is one click away. **The skills that come along as
references (§2) are in the same review**, each with its own summary, so nothing is installed that
the person was not shown. The skill installs in one click from the review. `purlis skill install` prints the same summary and asks the same question.

**The review is a reading aid, not a guarantee.** It is worked out from the files, so it lists
what they name, not what a script could compute at run time. What bounds an installed skill is
that it runs in the chat's sandbox (ADR 0067) and may only narrow a chat, never widen it
(spec #2762, #2771). There is no signature check until PE-36, which was the operator's call.

### 4. Nothing updates by itself

purlis tells the person when a source has a newer commit than a skill's pin: it asks the source
when the studio opens and on `purlis skill update`, and that check reads refs only. **A newer
commit is applied only when the person accepts it** through the same review, shown as a diff:
the summary's changes first (a new script, a new host, a new `!` line), then the files. Accepting
replaces the copy, moves the pin and is one commit. Nothing is fetched into the plane on a timer,
and declining leaves the skill as it was.

An installed skill can be edited like any other. It then says it differs from its pin, and an
update shows the source's change against the pinned commit beside the local edits, so an update
never overwrites an edit silently.

### 5. A private source signs in through `purlis secret exec`

A private source names a vault entry. Every fetch from it runs through `purlis secret exec`,
which hands git the credential for that one command: the token never reaches a chat, the
lockfile, the plane or purlis's own git runner, which strips credentials from what it starts.
Each person signs in with their ow### 6. On an org plane, the org decides what may be installed

Org planes add an allow/block list, a minimum source age and audit entries over this same
install (#2774).

7 days by default) is not offered, and
every install, update and removal is an audit entry. These are policies over the same install,
not a different one.

### 7. Not a registry, and why ADR 0041 still holds

ADR 0041 refuses a registry for extensions: once purlis resolves a name over the network it owns
a supply chain, and `purlis plugin install <name>` could never stand for the operator's consent.
Both reasons stay true for extensions, and M47's marketplace stays separate. A skill source is a
different thing on each count:

- **No name is resolved.** A source is a URL the person typed or picked, and an install is a
  copy of named folders at a named commit. purlis runs no index and vouches for no publisher.
- **What stands for consent is the review**, of the exact bytes, and then the commit in the
  plane. The command only opens the review.
- **A skill is not a program purlis runs.** An extension runs beside purlis; a skill is read by a
  chat that is already sandboxed, and can only narrow it.
- **It is standard practice.** Skill collections are already git repositories, and pinning a
  dependency to a commit in a lockfile is how every package tool does it.

### 8. Other kinds of source are extensions

A source that is not git (a registry, an artifact store) is an extension that declares the
**SkillSource** capability (ADR 0101). It answers two questions: what skills it has, and the
folder of one of them at an immutable revision. The revision takes the commit's place in the
lockfile. The review, the copy, the pin and the update are purlis's and are the same for every
source, so an extension can never install a skill the person did not review. The built-in git
source is SkillSource's first user.

## What was rejected

- **A registry or an index service** of skills run by purlis. It is the supply chain ADR 0041
  refused, and the marketplace is M47's.
- **Installing by link, or as a git submodule.** The skill would change under the person, and
  would not work offline.
- **Automatic or scheduled updates.** A pushed commit would run in chats before anyone read it.
- **Keeping the source and pin in `SKILL.md`'s frontmatter.** ADR 0099 keeps purlis's
  metadata beside the folder; an exported skill must not carry purlis's state.
- **A credential in the source's URL or in git's own credential store.** The first is committed
  in the lockfile; the second is a store purlis does not control and a chat could read.
- **Writing installed skills into a harness's own skills folder.** ADR 0099 rules it out for
  every skill.

## Consequences

- An installed skill works offline, and the plane's history says when it was installed, from
  where, at which commit, and who accepted each update.
- `skills.lock` and `skills.local.lock` are documented in `docs/plane-format.md` before the code
  that writes them, as the plane rule requires.
- Two people who install into the same project share one lockfile; two installs on different
  branches can conflict there, and the conflict is an ordinary save conflict.
- The review's summary is only as good as the reading behind it. A script that fetches and runs
  more at run time is caught by the chat's sandbox, not by the review, and the review says so.
- A non-git source waits for its extension, and for the confinement that lets a third-party
  extension declare a studio capability (ADR 0101).
- Signing (PE-36) can be added to the review later without changing the pin or the lockfile.

## Amendments

The amendments to this decision are files of their own in [`0100/`](0100/), one per
amendment, named `<yyyy-mm-dd>-<slug>.md`. A new amendment is a new file there and never a
section added to this record, so two pull requests that amend it do not touch the same lines
(#1982).
