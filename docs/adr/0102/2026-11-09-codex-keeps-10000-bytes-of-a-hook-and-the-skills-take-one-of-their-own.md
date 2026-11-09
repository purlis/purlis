# Codex keeps 10,000 bytes of a hook, and a Codex chat's skills take a hook of their own

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-09**, ticket #5841, left open by [Claude Code's limit holds the person's own skills, and Codex lists built-ins first](2026-11-08-claude-code-s-limit-holds-the-person-s-own-skills-and-codex-lists-built-ins-first.md). It changes how a Codex chat is told its skills (ADR 0063's briefing route), and corrects the limit [A chat's skill budget is its harness's](2026-11-02-a-chat-s-skill-budget-is-its-harness-s-and-the-setting-overrides-it.md) warns at for Codex.

## The question

A Codex chat is told its skills in the `SessionStart` briefing, as its last section. The 2026-11-08 record listed purlis's built-in skills first, on the guess that Codex, if it cut the briefing at all, would drop its end, as it drops the end of its own catalogue. Nobody had measured whether Codex cuts what a hook hands it, at what size, or from which end.

## What was measured

On **codex-cli 0.147.0**, the version `tests/fixtures/harness-versions/codex.txt` pins, from its npm builds (`@openai/codex@0.147.0-darwin-arm64` here, `-linux-x64` on CI's `Codex, measured` step, which agreed on every figure but the one noted), with no model and no credential: Codex was pointed at a model provider on a loopback port that refuses, so each turn fails at its first request, and what a hook's output became was read from the session's rollout (`$CODEX_HOME/sessions/**/rollout-*.jsonl`), which Codex writes before it sends anything. The hooks were armed with `-c hooks.SessionStart=[…]` and trusted with the records purlis computes for a sandboxed chat (`sandbox::codex::hook_trust`), so `codex exec` ran them. A hook answered `{"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": …}}` holding numbered lines of 44 to 46 characters, the shape of a briefing's skills.

- **Codex cuts a hook's output past 10,000 bytes, from the middle.** 9,963 and 9,995 bytes reached the conversation whole; 10,147 and 10,181 did not. A cut message keeps the same number of bytes of what was sent from its start and from its end, 4,922 on macOS and 4,938 on Linux (the rest of its 10,000 goes to the lines Codex adds, whose temporary path differs), with `…N tokens truncated…` in place of the rest, a first line `Warning: truncated output (original token count: N)` and `Total output lines: N`, and a last line `Full hook output saved to: <a file under the temporary folder>`, about 10,100 bytes in all. At 27,489, 139,889 and 1,268,889 bytes it was the same.
- **It counts bytes, not characters.** 150 lines of three-byte dashes, 7,839 characters and 11,139 bytes, were cut the same way, to 7,175 characters and 10,103 bytes.
- **The limit is fixed.** `gpt-5.5`, `gpt-5.2` (whose catalogue entries say `truncation_policy` `tokens` and `bytes`, both 10,000), a model Codex does not know, and `-c tool_output_token_limit=50000` each kept the same 10,102 characters of 27,489. No setting of Codex's moves it, so purlis has no number to hand Codex as it hands Claude Code one.
- **Each hook is cut apart, and a plain answer as JSON is.** Two hooks on `SessionStart` became two developer messages in the order armed, each with its own 10,000: four hooks of 7,249 bytes all reached the model whole, and beside a hook of 27,489 that was cut, a second of 4,489 was whole. A hook that prints plain text instead of Codex's JSON is cut the same way.
- **Codex's own catalogue is bounded another way** (`codex debug prompt-input`, `gpt-5.5`, a window of 272,000 tokens). At 100 skills of 100-character descriptions every description was whole (a catalogue of 17,519 characters); at 300 each was shortened evenly to about 23 characters (22,269); at 600, and at 2,000, the descriptions were empty and the list stopped at the 597th of the skills (21,659 characters), the last ones left out with nothing said in the list. That is 2% of the window at four characters a token, as the 2026-10-21 record read from the source; the 8,000 for an unknown window was not measured again.

The commands, as `crates/purlis-core/tests/it/what_codex_keeps_of_a_briefing_and_takes_in_mid_chat_is_measured.rs` runs them on CI's `Codex, measured` step:

```sh
CODEX_HOME=<home> HOME=<root> codex exec --skip-git-repo-check \
  -c 'hooks.SessionStart=[{hooks=[{type="command",command="<hook>",timeout=30}]}]' 'turn one'
CODEX_HOME=<home> HOME=<root> codex debug prompt-input
```

with `<home>/config.toml` naming `model_provider = "standin"`, its `base_url` on the refusing port, and `[hooks.state]` trusting the hook.

## The decision

- **A Codex chat's skills are a `SessionStart` hook of their own.** Codex is armed with a second `SessionStart` hook after the briefing's, `purlis hook sessionstart-skills`, that answers the skills section alone (`briefing::skills_part`); the briefing (`briefing::parts`) no longer carries it. A briefing alone passes 10,000 characters in an ordinary workspace: the one the chat that measured this was started with, the same blocks before the skills (the persona's memory digest, the sandbox note, the workspaces), was 13.7 KB. So the skills section, which sat after it with the extensions' sections and the kept reports behind it, was exactly the middle Codex cuts. In a hook of its own it has the whole 10,000.
- **Built-in skills stay first, and the index skill last of the described ones.** Within the skills hook, a cut keeps about the first and last 4,900 bytes, so the built-ins at its start are kept, and so is the end: the index skill, which names every skill the router hid, followed only by the one line naming the manual-only skills, where there are any. The 2026-11-08 order holds, for a reason the measurement gives instead of the one it guessed.
- **Codex's budget warning is the hook's cut, not the catalogue's.** purlis's list never enters Codex's own catalogue, so the 8,000 of the 2026-11-02 record does not bound it. Settings now says a Codex budget is past what Codex keeps when its characters pass 10,000 bytes less the listing's opening (`purlis_skills::LISTING_OPENING`), that Codex then cuts the middle of the list, and the number to set to keep every skill. The budget counts characters and Codex bytes, so a list in another script than plain English is cut before the warning says; the sentence is a floor, not a promise, and the order above is what holds past it. The adapter's own 6,000 sits under it. As before, nothing is refused for being large.
- **Codex asks once more to trust the hooks.** A new hook changes what Codex asks the operator to trust, once, as #1663 and #1086 did. A sandboxed chat's trust records are computed from the words it is armed with, so it trusts the new hook with the rest. **The cost:** an unsandboxed Codex chat on a machine that trusted the old hooks and not yet this one is told no skills at all, and nothing says so, until the person answers Codex's review prompt at the next start. Before, an untrusted briefing hook lost the briefing and the skills together, so this loses nothing a chat had before only where the old hooks were trusted; it is the price of the whole 10,000.

## What remains

- **The rest of a Codex chat's briefing is still cut past 10,000 bytes**, its middle lost: the persona's digest and the notes after it. That wants a ticket of its own, since what to move, split or shorten there is not a skills question.
- **Codex's own catalogue** (the person's skills and its plugins') is still shown beside the budget, never counted (the 2026-10-31 records). Its measured bound is above.
- The `Codex, measured` step prints every figure above on every pull request's run (`measured:` lines), so a new pinned version shows any change in its log.

**Built by #5841.**
