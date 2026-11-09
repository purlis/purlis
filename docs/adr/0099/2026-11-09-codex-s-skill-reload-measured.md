# Codex's skill reload, measured: a running chat takes in none of purlis's changes

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-09**, ticket #3002 (SK-31). [Changing a skill, one commit at a time, and the restart offer](2026-10-21-changing-a-skill-and-the-restart-offer-as-built.md) said Codex picks up an edited skill's text, inferred from the briefing route and not measured; SK-8 (#3044) then made Codex's reload `Nothing`, since the briefing names the copy purlis wrote in the set's folder. Neither was run on a real Codex.

## What was measured

On **codex-cli 0.147.0**, the version `tests/fixtures/harness-versions/codex.txt` pins, from its npm build, with no model and no credential (a provider on a loopback port that refuses, as [ADR 0102's amendment of the same day](../0102/2026-11-09-codex-keeps-10000-characters-of-a-hook-and-the-skills-take-one-of-their-own.md) describes). One `codex app-server` process on stdio held one conversation for three turns (`initialize`, `thread/start`, `turn/start` three times, each waited to `turn/completed`). It was armed with a trusted `SessionStart` hook that answers whatever a file holds when it runs and notes each run. After the first turn: the hook's answer was changed (`gamma v1` to `gamma v2`), as purlis's would be if the plane's skills changed; a skill in `$CODEX_HOME/skills` was edited (`alpha`) and another added (`beta`). The second turn followed 15 seconds later. Before the third, the client called `skills/list` with `forceReload: true`.

- **The `SessionStart` hook ran once.** One run after the first turn and still one after the third. No later turn's conversation held `gamma v2`: only the first turn's `gamma v1`.
- **Codex's own catalogue changes mid-chat only when something makes Codex read it again** (measured on macOS, inside purlis's sandbox). In the 15 seconds after the change there was no `skills/changed` notification, `skills/list` still named `alpha` as first described and no `beta`, and the second turn's conversation held nothing new. After the client's `forceReload`, `skills/list` named `alpha` v2 and `beta`, and before the third turn the conversation gained a second skills catalogue, with both. purlis's sandbox may keep Codex's file watcher (`app-server/src/skills_watcher.rs`) from hearing the change. CI's `Codex, measured` step takes this again on Linux, outside any sandbox of purlis's, and prints it on every run.

The commands are `crates/purlis-core/tests/it/what_codex_keeps_of_a_briefing_and_takes_in_mid_chat_is_measured.rs`'s `codex_takes_in_a_skill_changed_mid_chat`:

```sh
CODEX_HOME=<home> HOME=<root> codex app-server \
  -c 'hooks.SessionStart=[{hooks=[{type="command",command="<hook>",timeout=30}]}]'
# on its stdin, one JSON-RPC message a line:
#   initialize, initialized, skills/list, thread/start, turn/start "turn one"
#   (change the skills and the hook's answer, wait 15 s)
#   skills/list, turn/start "turn two"
#   skills/list {forceReload: true}, turn/start "turn three"
```

## The decision

- **A running Codex chat takes in no change to purlis's skills**, so its reload stays `Nothing` (`Harness::skills_reload`) and SK-7's restart offer is made for every change, as it is for Claude Code and opencode. purlis's skills reach Codex only through the `SessionStart` hook, which Codex runs once per session (and again on a resume, the restart the offer starts), so nothing purlis changes mid-chat can reach the model before a restart. That holds whatever Codex's own watcher does, since purlis never writes into a folder Codex watches.
- **The 2026-10-21 record's "Codex picks up the bodies" is withdrawn** for the reason SK-8 gave, now measured: no later turn was told anything new by the hook.
- A change to the person's own Codex skills is not purlis's to offer a restart for: those skills are the harness's own (`Route::Own`), never in the stamp.

**Measured for #3002.**
