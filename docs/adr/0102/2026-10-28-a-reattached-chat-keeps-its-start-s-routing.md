# A reattached chat keeps its start's routing

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-28**, ticket #3578 (SK-15f). It replaces "A reattached chat asks its router again" in [Router choice, extension routers and the routing record, as #3230 built them](2026-10-28-router-choice-extension-routers-and-the-routing-record-as-built.md).

## What is kept

The window's start of a chat keeps how it routed its set in the reopen record, as `chats[].routing`, beside `chats[].plane_skills` (#3466):

- `{"by": "built-in"}`: the built-in router, which was asked nothing. So is a chat the plane's skills did not reach.
- `{"by": "extension", "record": {…}, "answer": [{"skill", "why"}]}`: the routing record (§6), and the router's answer where it was taken. A chat that fell back keeps its record and no answer.

A chat on a runner keeps nothing, since its runner arms it.

## What a reattach does with it

A chat reattached from the session host still holds the listing its harness was handed at its real start. Its reattach routes its set by what was kept: Use skill…, the folder it would be handed, the arming and the restart stamp all say what that start's answer hid. **Its router is not asked again**, and no second `skill.routed` is written: the one its start wrote is in its run already, and its skill runs keep naming it.

A record with no `routing`, written before this, or one that does not read whole, reads as nothing kept. That chat's router is asked again, as before, and the answer is recorded under the run it is in. A chat started again, not reattached, is always routed anew.

## A kept answer is data

The kept answer is applied as every later routing of a chat's set applies its start's: only the skills the set still has for a router to hide are hidden. A kept answer can never hide a pin, a built-in skill, or a skill outside the chat's set. Its lines are read back as [`Line`]s and its error is cut and marked again. The reopen record sits in the state folder's `app/`, which no sandboxed chat writes (`Class::Integrity`).

**Built by #3578.**
