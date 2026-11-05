# A deleted skill's pin stays, and it is not installed

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-05**, ticket #5331. The record says only an accepted install or update writes an entry in the lockfile. A delete, one skill or a set, still writes none: the entry stays, so a skill brought back is pinned as it was, and a delete is a commit of the skill's own files.

An entry whose `SKILL.md` is gone is now read as a deleted skill's. Every reader through `lock::every` treats it as not installed: its source offers it again and asks nothing for it, and Delete all… does not list it. An install of that name at that level replaces the entry. Before this, a deleted skill still read as installed, and installing it again was refused.

A skill made by hand under a deleted skill's name at that level picks the old entry up, and reads as changed since install, so it starts unticked in Delete all…. That is the safe way for the mistake to fall, and no cheaper check tells the two apart.
