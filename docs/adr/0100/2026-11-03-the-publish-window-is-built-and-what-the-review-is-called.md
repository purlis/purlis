# The publish window is built, and what the install review is called on screen

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-03**, from the Skill studio's docs page (#2786), where the docs, this record and the window disagreed. The code is what holds; this note puts the record level with it.

## The publish window is built

The amendment of 2026-10-31, [Publishing a skill, and exporting one](2026-10-31-publishing-a-skill-and-exporting-one.md), ends with *"The window: Not built yet: the operator sees a prototype of the review page and the export dialog first."* That was true when it was written and stopped being true when #4509 (#2781) merged: the window has both.

- A skill's **⋯** menu has **Publish to a source…** and **Export…**.
- **Publish to a source…** opens a review page, *Publish <skill> to a source*, with the **Source** picked, what stops the publish, **Where it goes** (**Goes into**, **New branch**, **Lands in**, **Title**, **Commit names**), the files that go, what stays behind and the grades that go in the request's description. Its one way on opens the draft request. What it stops on, and that only a person publishes, are the 2026-10-31 amendment's, unchanged (`app/src/SkillPublish.tsx`).
- **Export…** opens a dialog that saves **A zip** or **A folder**.

## "Install review" is a name, never a heading

§3 calls the page shown before anything is copied the **install review**, and the docs and the changelog keep that name. On screen the page has no such heading. **Review install** opens it, and its heading is **Install** *<skill>* **from** *<source>*, or **Install** *N skills* **from** *<source>* for Install all; an update's is **Update** *<skill>*. Its rows say **Lands in** for the level and **Reaches** for a project skill's grant ([ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md)). The glossary (`CONTEXT.md`) has an entry for it that says so.
