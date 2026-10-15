# Amended 2026-10-14: a declaration describes the profiles of its harness (#2031)

The profiles spec (#2030), from the operator's rulings of 2026-10-14, makes a profile one
identity of a harness: its settings, its sign-in and its conversations. Each declaration, built-in
or a project's, now says four facts about the profiles of its harness, in the same file and under
the same refusals. **The key names were confirmed by the operator on 2026-10-14.**

- **`[sign_in] kinds`**: the kinds of sign-in it supports, from `subscription` (a plan signed in
  through the harness's own flow, which the harness keeps and purlis never reads, copies, stores
  or relays), `api-key`, `cloud` and `local`. None listed: none is offered.
- **`[sign_in] kept_from_commands`**: whether its credential files can be kept from the commands
  it runs. It is answered as a capability is (§6): `"yes"`, `"no: <the reason>"` or `"unknown"`,
  and silence is unknown, which reads as no.
- **`[folders] config_env`** and **`[folders] data_env`**: the variables purlis sets to a
  profile's folder, each one variable's whole name, and for a harness purlis does not ship one
  under §5's refusals and in its own `env` namespace. None declared: purlis sets none.
- **`[conversation]`**: how a conversation leaves a device and comes back, for a chat that moves.
  A `file` is copied: a path under `{config}/` or `{data}/` (the folders the variables name),
  where a `*` part, or a `*` inside a part, is kept as it was found, `{cwd_key}` is the folder
  named after the chat's working directory, made again on the device it comes back to, and
  `{id}` is the session; `with_forked_from` brings the session it was forked from too. Otherwise
  the harness's own `export` words print session `{id}` and its `import` words take that file;
  their words take §5's shapes. Which one is the keys it gives. The session then resumes by
  `[session] resume`. Absent: the conversation does not move.

The built-ins declare all four. Claude Code supports `subscription`, `api-key` and `cloud`, sets
`CLAUDE_CONFIG_DIR`, and moves a transcript by its path,
`{config}/projects/{cwd_key}/{id}.jsonl`, where `{cwd_key}` is the working directory's absolute
path with every character other than an ASCII letter or digit written as `-`. Codex supports all
four kinds, sets `CODEX_HOME`, and moves its session file and the one it was forked from. opencode
supports all four kinds (a Claude plan never, #2039), sets `XDG_CONFIG_HOME` and `XDG_DATA_HOME`,
and moves a conversation by its own export and import. opencode keeps its files in an `opencode`
folder under each XDG folder (its `packages/util/src/global-roots.ts`), where Claude Code and
Codex keep theirs in the folder the variable names; `OPENCODE_CONFIG_DIR` only adds a folder it
searches, so it is not the one set. The XDG variables reach the commands a chat runs as well, as
every variable set on the harness does. Codex and opencode answer `kept_from_commands` no: each
is wrapped with its commands in one sandbox. **Claude Code answers unknown until #2037 measures
it.**

A local replacement of a built-in's facts (#968) keeps these four as the built-in's: they are how
purlis runs a harness it ships.

**`purlis profile list`** shows the sign-in purlis can see each profile has, and `unknown` where
it cannot tell; it never infers a subscription. What it sees, until a profile names its own kind
(#2034, #2036): a harness that supports one kind alone gives its profiles that kind; for a
harness purlis ships, a cloud switch set in the profile's `env` or passed by `[chat_env] pass`
(Claude Code's `CLAUDE_CODE_USE_BEDROCK`, `_VERTEX`, `_FOUNDRY`) gives `cloud`, and a provider's
key variable there (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, …) gives `api-key`, where only one of
the two is seen. **To be confirmed:** which variables are read is a list in purlis's code
(`harness_declaration::sign_in_variables`), not yet declaration data, and the encoding
`{cwd_key}` stands for was read from Claude Code's behaviour, not measured by TS1.
