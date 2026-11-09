# An org's project holds what it installs to its lists and an age, and audits it

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md) §6, 2026-11-09**, ticket #2774 (SK-11), spec #2762 stories 51 to 53. §6 said org planes add an allow/block list, a minimum source age and audit entries over the same install, without saying what an org plane is, where the lists live, how one install gets past the age, or which log the entries go to. purlis had no notion of an org plane yet (the policy layer, SD-14 #697, is still open). This records what was built.

## What makes a project an org's

**Its `charter.toml` holds a `[skill_policy]` table** (`docs/plane-format.md`). The issue's words are "a project keeps an allow/block list", and the committed settings file is where a project keeps what the team decides: a change to it is a commit the team reviews, and a chat may never write it. When SD-14 brings an administrator's policy, it can set or lock the same table; nothing here waits on it.

- **Only the shared file is read.** A `[skill_policy]` in `charter.local.toml` is never applied: it refuses every install and update until it is taken out, so one machine cannot loosen the team's controls, nor quietly think it tightened them.
- **A table that cannot be read refuses**, failing closed, as `[[skill_sources]]` does, and so does a `charter.toml` that does not parse and names `skill_policy` anywhere: its controls cannot be told from none.

## The lists

`allow` and `block` each hold `{ source = "<url>" }` or `{ extension = "<id>" }`, and `skill = "<name>"` to name one skill of it. A block wins. **Absent, `allow` allows every source not blocked**; given, it allows only what it covers, a skill brought along included. Failing closed on an absent `allow` was rejected: a team that wants only the age would have to list every source it uses, and would stop using the age instead. URLs compare without a trailing `/` or `.git` and with any case in scheme and host.

## The age

**`min_age_days`, 7 by default**, is held against the commit's committer time, read from the commit object the review fetched. It is asked at the review, so the review says why and when it can be installed ("… was committed 2 days ago, and this project installs only commits at least 7 days old: it can be installed in 5 days"), and again at the install or update, so a review accepted before the age was set installs nothing. An extension's source has no time to give, so it is refused while the age is on, unless exempt.

**The committer time is the commit's own word.** Whoever makes a commit sets it, so a backdated commit passes. A source's push time is not in git, and a forge's API differs per forge and is unavailable for a folder; the age is a guard against the ordinary case, a malicious commit pushed and installed the same day, and the allow list is the control that does not depend on the author's word.

**One install gets past the age only through `min_age_exempt`**, a list of full commits (or revisions) in the same table. A flag on `purlis skill install` was rejected: the age is the org's control over a member's install, and a flag every member can pass is not a control. An exempt commit is a reviewed change of the team's file, names exactly one commit and so never lets a later push through, and is the idiom package managers already use (pnpm's `minimumReleaseAgeExclude`), narrowed to a commit because a source's name says nothing about what was pushed to it.

## The audit

Every install, update and removal of a skill installed from a source is **one event in this machine's event log**, the audit's source (ADR 0075 §1): `skill.install`, `skill.update`, `skill.remove`, each with the level, the skills' names, the source and the commit. The source is an `https://` URL without query or fragment, `extension:<id>`, or `local:<label>` for a folder of this machine, never its path (ADR 0075 §4). It is made durable **before** the change, as an applied proposed change is, so no change lands unrecorded; a change that then fails leaves its entry, so an entry says what was done or attempted, and the plane's history says which. A project without the table records nothing new.

- **Restoring a deleted skill that was installed from a source is an install**: it is held to the lists (not the age: its commit is the one it was pinned to) and audited as `skill.install`.
- **Undoing an update set moves each pin back**, audited as `skill.update` to the earlier pin.
- **A lockfile that cannot be read refuses a removal** on an org's project, which could not tell whether the skill was installed from a source.
- **No registry yet.** ADR 0075 §4's registry lives in the `charter-audit` crate (AU-2), which is not built; the three kinds are constants beside the other audited kinds (`trust.proposed.apply`, the dispatch grants), to be registered with them.

**Only the window writes the event log** (session protocol 1.8: a scope that could write it could record an act no person made). So on an org's project, `purlis skill install | update | rm` of an installed skill checks the lists and the age, says either refusal, and then refuses the change itself, naming the Skill studio. The core holds this: a change on an org's project with nothing to audit it is refused, whoever asks. Recording a terminal's change without the window (a journal the host reads, as the kill switch's) is a follow-up.

## What it does not do yet

- The window shows a refusal where it shows the review's other failures. A view of the controls themselves, and of the audited acts, needs a prototype first and is a follow-up.
