# Amended 2026-10-28: a grade runs on the model the person picks

Ticket #3240 (SK-60), from spec #2762 decision 22 ("on a chosen harness and model") and the spec's
"results tied to the skill's commit, harness and model". It adds to §3, §4 and §5, and ends the
interim of the amendment of 2026-10-27, under which a grade ran on whatever model its profile's
words named.

**The plan offers the models the profile's harness can run**, beside the profile's own model,
which stays the default. Which models those are is the harness adapter's word
(`grade_models`), with nothing in the neutral core: Claude Code's are its three family aliases,
`opus`, `sonnet` and `haiku`, each its family's newest model, so the list never goes stale with a
release. A full model id stays the profile's to name in its command. Codex and opencode offer none
while they cannot grade.

**A picked model reaches the chat by the adapter's own words** (`model_words`), pure, put after
the profile's own words, so the model picked is the last the line names and outranks the
profile's (Claude Code's `--model`, which takes the last one). A model the adapter does not
offer is refused with a sentence, so nothing but a name the harness takes reaches the line. The
dollar limit is priced from the line as it starts (`grade_model`), so from the model picked.

**Each grade says the model it ran on.** The eval record gains `model`: the model its chats
reported, as their harness's `SessionStart` names it (ADR 0087 §6), the first that said one, by
the harness's own id. It is checked as `priced.model` is, and a row that holds anything else is
skipped. The Grades panel's "Ran on" says it beside the program, and `purlis skill grade` beside
the chats. A row written before has none.

## Rejected

- **A model list kept by purlis, priced or not.** Each harness's flag takes names of its own, and
  the adapter is where its words live; the price table stays a bound, never an offer.
- **Recording the model picked rather than the one reported.** The spec ties results to the model
  the chats ran on, and a harness may remap an alias (`ANTHROPIC_DEFAULT_OPUS_MODEL`) or fall back:
  what it reported is what ran.
