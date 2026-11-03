# A publish asks the forge for write access first, and only warns

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-03**, part of #4342, by the operator's ruling of the same day: publishing through a fork, and a grade a source carries (#4346), move to M65.1. Only this check is built now.

The amendment of 2026-10-31, [Publishing a skill, and exporting one](2026-10-31-publishing-a-skill-and-exporting-one.md), says *"A person who cannot push is told so."* That was said only after git's push was refused.

- **The review asks first.** Before anything goes, the publish review reads the forge's answer to whether the person's account may push to the source: GitHub's `permissions.push`, or a GitLab access level of Developer or above (the `about` read the forge already makes).
- **Where the forge says no, the review says so, with Export beside it.** The publish page shows it at the top, and `purlis skill publish` prints it before it asks.
- **It never refuses on that answer.** GitLab says no to an instance's admins and to access through a group the project is shared with. The account the API answers for need not be the one git pushes as, either. So the button stays and the push answers for itself, as before.
- **A review that something already stops does not ask.**
- **A fork is still not made.** That is #4342's, in M65.1.
