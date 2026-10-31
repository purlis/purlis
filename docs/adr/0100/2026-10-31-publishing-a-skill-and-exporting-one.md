# Publishing a skill, and exporting one

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-10-31**, ticket #2781. Spec #2762, decisions 21, 44 and 57.

## Only a person publishes

- `purlis skill publish` prints everything that would go, then takes a yes from a real terminal. It has no flag that skips the question, unlike `install --yes` and `report --yes <digest>`, since either would let a chat publish.
- It is refused with no terminal on stdin and stderr, and inside a chat, before the source is asked anything. The release floor also denies it from an unattended run, in a table of its own (`PUBLISH_SINCE`), since `PUBLISH_FORGE` is replayed against a frozen recording that stays as it is.
- What goes is what was reviewed: `publish` reads everything again and refuses where any of it moved.

## What goes, and what stops it

- A skill goes with the project's skills it refers to. Links, files named like credentials, files the project's git ignores and names purlis would not install stay behind, and are said.
- Anything credential-shaped stops a publish and an export, with no override. Personal data, a home folder's path and a skill kept on this machine stop a publish alone, since an export stays with the person until they share it.
- Grades go as a summary in the request's description, never in the skill's folder: date, version, harness, grader, pass rates, runs and how it ended. A chat's text, a path, a cost, a profile and a device never go.

## Where it goes

- A new branch on the source itself, named by the digest of what it holds, by one refspec that is never forced, and a draft request from it. A person marks it ready. A person who cannot push is told so; a fork is #4342.
- A branch an earlier publish left that holds the same files is used again, and one that holds anything else is refused. A closed or merged request does not count as already published.
- The review names whom the commit names, from the project's git config.
- The commit is built from git's objects over the source's tree at the reviewed commit, with no checkout.

## Export

A new folder or zip of skill folders, never written over anything. The same files make the same zip.

## The window

Not built yet: the operator sees a prototype of the review page and the export dialog first.
