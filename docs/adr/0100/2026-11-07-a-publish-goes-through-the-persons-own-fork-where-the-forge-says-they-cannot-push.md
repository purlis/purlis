# A publish goes through the person's own fork where the forge says they cannot push

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-07**, ticket #4342, by the operator's ruling of the same day. It replaces the last line of [A publish asks the forge for write access first, and only warns](2026-11-03-a-publish-asks-the-forge-for-write-access-first.md), *"A fork is still not made"*, and that amendment's warning where the forge says no.

A fork is a lasting repo on the person's account, and it is public when the source is public. So it is made only on a person's yes to a review that named it and said it would be made.

## When the branch goes on a fork

- **Only where the forge says the account may not push to the source.** Where it says yes, or does not say, the branch goes on the source itself as before, and the push answers for itself.
- GitLab says no to an instance's admins and to access through a group the project is shared with. Such a person gets a fork they did not need. That is accepted: a fork is never wrong, only spare.

## Whose fork, and which

- **The account's own namespace only**, never an organisation's or a group's, at the source's name.
- **A fork the person has is used again**, but only one the forge says is a fork of this source: on GitHub one the source's forks list as the account's own, whatever it was renamed to; on GitLab one whose `forked_from_project` is the source by id.
- **A repo of theirs at that name that is not the source's fork stops the publish**, with a sentence. purlis never renames to get around it.

## What the review says, and what a yes covers

- The review names the fork, says whether it will be made, and says who can read it, in the words purlis uses before it sends (*"PUBLIC: everyone can read what is sent"*).
- **The fork is the forge's default visibility**, and purlis never changes it.
- Where the branch goes is part of what a yes is a yes to (`PublishReview::accepted`). A yes to a branch on the source is never a yes to making a fork: when the forge's answer moved between the review and the press, the publish is refused and nothing is made.
- The fork is made in one place only, after every other check has passed and before the push. It is not made from a review, and not from a chat: `purlis skill publish` still takes a person at a terminal, and the window's command is the window's alone.

## After it is made

- **A fork stays, and purlis never deletes one.** Where the push or the request fails after the fork was made, the message says the fork was made and stays, and a publish again uses it.
- Both forges copy a fork in the background. The forge seam waits for GitLab's `import_status` to be `finished`, for at most two minutes, and the publish then waits until git answers on the new fork, for at most a minute, before anything is pushed.
- The fork's branch is read and pushed with the person's sign-in for the host, so a private fork is reached as the push reaches it.
- The push to the fork uses the same credential helper as a push to the source: the person's purlis sign-in for the host. No token passes through a chat.
- The draft request is opened into the source from the fork's branch: on GitHub with a `login:branch` head, on GitLab on the fork with the source as `target_project_id`. A request already open from the same files on the same fork is answered as before. A request from another fork's branch of the same name is not.
