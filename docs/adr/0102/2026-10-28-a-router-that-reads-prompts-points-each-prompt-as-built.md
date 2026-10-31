# A router that reads prompts points each prompt, as #3231 built it

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-28**, ticket #3231 (SK-15c), after #3230's amendment of the same day. ADR 0102 §3 says that a router that reads prompts is asked on each prompt which skills suit it and are not in the listing, and that purlis adds them to the prompt's hook context as a short pointer. This records which router is asked, where the prompt goes, what the pointer says, and what the record holds.

## Which chats

**[The operator's ruling of 2026-10-31](2026-10-31-a-router-reads-prompts-only-once-the-person-turns-that-on.md) adds a condition: the person turned reading on for that router.**

A chat's prompts are routed only where all of these hold at its start:

- its persona or workspace chose an extension's router (#3230);
- that router's approval said it reads prompts: `"reads-prompts": true` in its `contributes.skill-router`, which the approval prompt names ("it reads every prompt of every such chat");
- the chat's list leaves skills out, which the index skill it was handed names.

Such a chat is started with `PURLIS_PROMPT_ROUTER=1`. Its prompt hook hands the prompt over only where that is set, so **no other chat's prompt leaves its hook**. The executor holds the same line: a prompt question to a router that does not declare it reads prompts is refused before anything starts.

## Where the prompt goes

**The hook hands the prompt to the app, over the chat's own authenticated hook line** (`Ask::Prompt`), cut to 16 KiB there, and the app asks the router. A chat a runner's host composed is routed there, never by this machine. Asking from the hook itself would start the router inside the chat's own sandbox, where the router's confinement cannot be applied a second time, so a sandboxed chat (the default) would never be routed. The app's executor asks it confined, as at the chat's start.

The prompt goes no further than the router. The app writes it nowhere, a debug dump of the ask says nothing of it (`PromptAsk`, as SK-38's `PromptMarks`), and the routing record holds no word of it. **Nothing of the chat beyond the prompt goes with it**: no chat id, no prompt salt, no name or path. The prompt is cut to 16 KiB.

The question is protocol 3's studio question `prompt`:

```json
{"ask": "prompt", "prompt": "…", "skills": [{"name": "deploy", "line": "…"}]}
```

`skills` are exactly the skills the chat's list left out at its start, each with its line in the index skill, kept with the start (`Routed::left_out`). The list a harness takes at its start never changes during the chat, so these never do either. The answer is `{"pointed": ["<name>", …]}`: at most five, each one of `skills`, once. A router names skills and gives no words of its own here.

## What the prompt is handed

Each pointed skill by its name and **purlis's own line from the index**, never the router's words, and that `skill_load` loads it:

```text
This chat's skill router suggests these skills for this prompt. They are not in its list: load one with purlis's `skill_load` tool by its name before you follow it.
- `release`: Cuts a release.
```

A skill loaded so is loaded with `skill_load`, so its run has an exact start.

## 100 ms, and the record

**The same 100 ms as at the chat's start**, counted from before the router's slot is held. The hook waits on the app at most 400 ms in all. Past the deadline, on an error, or on an answer purlis does not take, the prompt goes on with no pointer. Chatting never waits on a router.

Each prompt routed is a `skill.routed` record under the chat's run, written before the prompt's own event. It has `prompt: true`, `pointed` (the names), and `fell_back` with `error` where it fell back. Every skill run the turn opens names it as `prompt_routing`, with `prompt_routing_fell_back` where it fell back, beside the start's `routing`. A slow prompt does not say the chat's list fell back: the start's routing is what the list was made by.

**A router slow at a chat's start still routes its prompts**: what it is asked about is what the built-in router left out in its place. Whether it reads prompts is read once its start's question is over, so that read is never inside its 100 ms, and never again on a prompt; the executor still refuses a prompt to a router whose approval says it reads none.

## A harness without a prompt hook

Every harness purlis ships has one: Claude Code's and Codex's `UserPromptSubmit`, and opencode's `chat.message` in purlis's plugin (`Harness::prompt_hook`). A harness without one would get the listing and `skill_find` alone. Its plane skills' compatibility badges would say so: "No router points a prompt at it here: this program has no prompt hook, so the chat finds it in its list, the index skill or with skill_find."

**Built by #3231.**
