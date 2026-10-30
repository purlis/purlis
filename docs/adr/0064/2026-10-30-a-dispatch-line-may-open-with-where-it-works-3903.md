# Amended 2026-10-30: a dispatch line may open with where it works (#3903)

#3903 (from #3897, concern 4) gives `--in` a third spelling, `worktree:<repo>`, for a chat whose
workspace keeps its clones in subfolders. That chat is told to write `--in worktree:<repo>`.
A line that opened with `--in` matched none of the V98b rules (amendment of 2026-10-07), so
Claude Code asked about it, while the same dispatch written with `--name` first ran without a
prompt.

- **One more rule, by its flag:** `Bash(purlis dispatch --in *)` joins `DISPATCH_ALLOW`, beside
  `--name`, `--to` and `--profile`. A line that opens with a flag is still the task and cannot
  reach a subcommand, so the rule allows nothing new beyond the task itself.
- **What does not change.** Consent is still the app's dispatch grant. A dispatch beside another
  command is still refused as a rider (amendment of 2026-10-29 for #3785). An operator's own
  `ask` or `deny` still wins. `--in=worktree`, with `=`, matches no rule and is left for the
  harness to ask about. Nothing is added for Codex or opencode.
