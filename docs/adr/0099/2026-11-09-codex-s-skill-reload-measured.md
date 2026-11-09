# Codex's skill reload, measured: a running chat takes in none of purlis's changes

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-09**, ticket #3002 (SK-31). [Changing a skill, one commit at a time, and the restart offer](2026-10-21-changing-a-skill-and-the-restart-offer-as-built.md) said Codex picks up an edited skill's text, inferred from the briefing route and not measured; SK-8 (#3044) then made Codex's reload `Nothing`, since the briefing names the copy purlis wrote in the set's folder. Neither was run on a real Codex.

## What was measured

On **codex-cli 0.147.0**, the version `tests/fixtures/harness-versions/codex.txt` pins, from its npm build, with no model and no credential (a provider on a loopback port that refuses, as [ADR 0102's amendment of the same day](../0102/2026-11-09-codex-keeps-10000-bytes-of-a-hook-and-the-skills-take-one-of-their-own.md) describes). One `codex app-server` process on stdio held one conversation for three turns (`initialize`, `thread/start`, `turn/start` three times, each waited to `turn/completed`). It was armed with a trusted `SessionStart` hook that answers whatever a file holds when it runs and notes each run. After the first turn: the hook's answer was changed (`gamma v1` to `gamma v2`), as purlis's would be if the plane's skills changed; a skill in `$CODEX_HOME/skills` was edited (`alpha`) and another added (`beta`). The second turn followed 15 seconds later. Before the third, the client called `skills/list` with `forceReload: true`.

- **The `SessionStart` hook ran once.** One run after the first turn and still one after the third. No later turn's conversation held `gamma v2`: only the first turn's `gamma v1`.
- **Codex's own catalogue is taken in mid-chat, by its own watcher** (`app-server/src/skills_watcher.rs`). On Linux, on CI's `Codex, measured` step, outside any sandbox of purlis's, a `skills/changed` notification came within the 15 seconds, `skills/list` named `alpha` v2 and `beta`, and before the second turn the conversation gained a second skills catalogue with both. On macOS inside purlis's sandbox the watcher heard nothing: no notification, `skills/list` unchanged, and the second turn told nothing new, until the client's `forceReload` before the third turn brought the same second catalogue. So it is the watcher that carries the change, and a sandbox that keeps file events from Codex keeps it from a running chat.

The commands are `crates/purlis-core/tests/it/what_codex_keeps_of_a_briefing_and_takes_in_mid_chat_is_measured.rs`'s `measures_what_codex_takes_in_of_a_skill_changed_mid_chat`:

```sh
CODEX_HOME=<home> HOME=<root> codex app-server \
  -c 'hooks.SessionStart=[{hooks=[{type="command",command="<hook>",timeout=30}]}]'
# on its stdin, one JSON-RPC message a line:
#   initialize, initialized, skills/list, thread/start, turn/start "turn one"
#   (change the skills and the hook's answer, wait 15 s)
#   skills/list, turn/start "turn two"
#   skills/list {forceReload: true}, turn/start "turn three"
```

What it printed on CI (Linux x64, codex-cli 0.147.0, 2026-11-09, the pull request of #5841):

```text
measured: skills/list before the change: ["alpha: Alpha v1."]
measured: notifications in the 15 s after the change: {"skills/changed"}
measured: skills/list after it: ["alpha: Alpha v2, edited mid-chat.", "beta: Beta, added mid-chat."]
measured: skills/list with forceReload, after turn two: ["alpha: Alpha v2, edited mid-chat.", "beta: Beta, added mid-chat."]
measured: SessionStart hook runs: 1 after turn one, 1 after turn three
measured: conversation: skills catalogue ["- alpha: Alpha v1."]
measured: conversation: hook output "BRIEFING gamma v1"
measured: conversation: user: turn one
measured: conversation: skills catalogue ["- alpha: Alpha v2, edited mid-chat.", "- beta: Beta, added mid-chat."]
measured: conversation: user: turn two
measured: conversation: user: turn three
```

And on macOS, inside purlis's sandbox, the same day: no notification, `skills/list` after the change still `["alpha: Alpha v1."]`, and the second catalogue only after `forceReload`, before turn three.

## The decision

- **A running Codex chat takes in no change to purlis's skills**, so its reload stays `Nothing` (`Harness::skills_reload`) and SK-7's restart offer is made for every change, as it is for Claude Code and opencode. purlis's skills reach Codex only through the `SessionStart` hook, which Codex runs once per session, and again with `source` `resume` on `codex resume <id>`, the restart the offer starts (measured on the same version for #27, `harness::codex`'s arming notes), so nothing purlis changes mid-chat can reach the model before a restart. That holds although Codex's own watcher takes in its own skills live (above): purlis never writes into a folder Codex watches, and the set's folder it names is not one.
- **The 2026-10-21 record's "Codex picks up the bodies" is withdrawn** for the reason SK-8 gave, now measured: no later turn was told anything new by the hook.
- A change to the person's own Codex skills needs no restart where Codex's watcher hears it, and is not purlis's to offer one for: those skills are the harness's own (`Route::Own`), never in the stamp.
- **SK-7's restart offer for Codex is unchanged on purpose**: it already matched what was measured.

**Measured for #3002.**
