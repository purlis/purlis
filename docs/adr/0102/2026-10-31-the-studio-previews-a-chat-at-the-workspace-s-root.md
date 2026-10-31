# The studio previews a chat at the workspace's root

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-31**, ticket #4215, under the operator's spec #2762. It settles what [The router never counts a harness's own skills](2026-10-31-the-router-never-counts-a-harness-s-own-skills.md) left open: the studio routed its preview with the repo skills of every clone in the workspace, which no chat's start counts.

## The decision

- **The studio's hidden marks preview one chat: a chat at the workspace's root** (or the project's, at its root), on each harness, as the persona the studio is filtered to. That is where a chat of the workspace starts unless it is started in a clone. Such a chat is in no clone, so its start counts no repo skill against its budget, and neither does the preview. `harnesses_in_the_studio` routes the plane's set beside only the harness's own skills, which count against nothing, through `skills::as_a_start_routes`, the function a start routes through. The marks are then exactly what that chat's start hides.
- **Every clone's repo skills stay in the harness's set** for what is not routing: their rows under the harness, their clashes with plane skills, and the plane skills a repo leaves out of a chat in that clone. Each of those names its repo, so listing every clone's is no claim about one chat.
- **The preview routes by the built-in router, always.** An extension's router is asked only at a chat's real start (§5, §6). Asking it on a read of the studio would spend its 100 ms deadline on every read and push against the 8,000 ms open cap. Its answer is per chat, so there is no one recorded choice to show either. The Routing page's **Preview** line says this in one sentence, `StudioRouter::preview`. It names the place previewed. Where an extension is chosen, it says that the marks are the built-in router's choice. In a workspace, it adds that a chat in a clone counts that clone's repo skills too, and that its own Skills view says what it hides. Each chat an extension routed is listed on that page with what it hid, as before.
- **A chat in a clone is not previewed one by one.** Its own Skills view and Use skill… already read its real set, routed by its start. A clone picker on the harness page would be a new surface, and it waits for its own design (#4227).

**Built by #4215.**
