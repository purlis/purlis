# Amended (2026-11-05): a prompt that runs one skill starts it as its harness does (#5186)

Ruled by the person on 2026-11-04 (Q5). This replaces what the record says about naming a skill
"in words, not by a harness's `/slash` syntax", for the prompts purlis writes. It stays true for a
persona's own action: that file is one text for every harness, so it is typed as written.

A prompt purlis fills in to run one skill is one neutral value, `skillprompt::SkillPrompt`: the
skill, its origin, the words that follow it, and a sentence. Each harness adapter says whether its
own start reaches a skill of that origin (`own_start_reaches`), and renders it. It is still typed
and never sent.

| Harness | A built-in skill | A skill of the person's set | Why |
|---|---|---|---|
| Claude Code | `/purlis:<name> <words>` | `/skills:<name> <words>` | Its plugins' skills, by plugin. |
| opencode | `/<name> <words>` | `/<name> <words>` | Every skill it holds is a slash command. |
| Codex | the sentence | the sentence | purlis only lists these (ADR 0063), and `$<name>` resolves only among skills Codex found itself. |
| a harness with no adapter | the sentence | the sentence | |

**A direct start holds its run.** A harness expands it into the skill with no call to its Skill
tool, so no tool call would open the run. Each one is held as Use skill… holds its own, says what
started it, and waits however long the person reads first.

**Try it falls back to the sentence** for a draft that cannot be started by its folder's name: a
frontmatter name of another spelling, `user-invocable: false`, or named `arguments` the words would
fill one by one. A slash command the harness does not answer to ends the turn before its model
reads a word.

**Not changed:** Smart close and a grade's chats. Both are sent by purlis rather than typed for
the person to read, and a person's edit cannot rescue a start that fails, so smart close keeps its
sentence (ADR 0064). A grade sends its fixture's query as written. Handoff fills in no prompt that
starts a skill: its first message is the asking chat's brief.
