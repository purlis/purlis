# Amended (2026-10-14, the operator's ruling T-1): a declared tool may receive several values

Ruled by the operator. This extends the 2026-10-11 and 2026-10-12 paragraphs of section 1 on a
persona's tools (D-PE-ruling-1, D-PE-ruling-2, D-PE-browser-1).

- **A tool may be handed several values from its one vault**, named under `values`, each with
  its variable, key and hand-over: `aws = { vault = "cloud", values = { AWS_ACCESS_KEY_ID = {
  key = "aws-access-key-id", as = "env" }, AWS_SECRET_ACCESS_KEY = { key =
  "aws-secret-access-key", as = "env" } } }`. The program reads each value from its variable or
  file itself; purlis adds nothing to its arguments. The one-value form (`as` and `key`) stays, unchanged in
  meaning and in its Allow; a tool written both ways is refused with a sentence. A `values`
  table of one is that same tool.
- **Each variable is held to the one-value form's rules** (a variable's name, none of the names
  that steer how programs are found or loaded, none of purlis's own), no two values use the same
  variable in any case, each `as` is `env` or `file`, and a tool takes at most 16 values: enough
  for any one program's credentials, and few enough that the line a person allows can be read
  whole.
- **The per-machine Allow covers the whole declaration.** The tool's one line, which the digest
  is over and which the Notice, Settings › Sandbox and the audit show, names every value's
  variable, key (quoted) and hand-over, in the order of their variables, so the order they are
  written in changes nothing and any other change asks again. On the Granted row and in the
  audit, tools are kept apart by semicolons.
- **The shim hands every value**: `purlis secret exec <vault> --file A=<k1> --env B=<k2> … --
  <absolute program>`. The browser login's two files are the same kind of hand-over: one path
  for every tool.
- **The brokered run checks the chat's request against the declared set exactly**: the tool's
  vault, every one of its values and nothing else, each variable paired with its key and handed
  the way it is declared, in any order, and no `--dotenv`. Anything else is refused with a
  sentence naming the declared line and the command that runs it. No refusal names a value.
- purlis logs no value and no refusal names one; a `file` hand-over keeps the one-value rules
  (0600, removed when the run ends), and output is masked for every value. Policy's `persona-tools` still forbids
  every tool, and tools stay local to this machine's chats (T-2).
