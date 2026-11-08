# The studio's previews list an identical pair once

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, ticket #5843, under the operator's spec #5714. It closes the item about the studio that [One skill from two sources takes one slot](2026-11-08-one-skill-from-two-sources-takes-one-slot-and-differing-copies-are-both-listed.md) left open, and keeps [The studio previews a chat at the workspace's root](2026-10-31-the-studio-previews-a-chat-at-the-workspace-s-root.md).

## The question

A chat's start leaves out a plane skill whose copy the harness has itself, file for file, and lists the harness's copy in its place. The Skill studio routed its preview of each harness's chat from the plane's set alone, so it showed both copies, counted the plane's against the budget and marked it as a router would, where the chat itself lists it once.

## The decision

- **Each harness's preview leaves out what that harness's chat leaves out.** The studio reads each harness's own skills already. It drops from the plane's set every skill that harness has the same copy of (`SkillSet::without_copies_of`, with purlis's own plugins left out of the comparison as at a chat's start, `skills::the_persons`) before it routes its preview at the workspace's root and in each clone. The budget, the hidden marks and what is listed are then the chat's.
- **The copy left out is listed under its harness, naming the copy kept.** `HarnessInTheStudio::same_left_out` holds those rows, beside the rows a repo leaves out (SK-53). On Harness skills each is a row marked _Listed once_, whose line is the reason the chat is told: _the same as your own copy, which is listed in its place_. `purlis skill list` prints them under the harness, and `purlis skill show` lists them among the copies of the name.
- **The plane's own list is unchanged.** It is the plane's set for every harness at once, so it still lists the plane's copy. One harness's copy says nothing of another harness, which may have no copy and get the plane's.
- **Copies that differ** stay in the preview, both listed, as at a chat's start.

## What remains

- A Codex chat's briefing hook still passes no harness skills, so its briefing names both copies of an identical pair, as the 2026-11-08 record says.

**Built by #5843.**
