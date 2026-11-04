# Amended 2026-11-04: a Secret ask runs without a prompt (#5072)

Ruled by the operator on 2026-11-04, on the terms of the amendment for #3829: **`secret_ask`
joins `chattools::OWN_SCOPE_TOOLS`, beside `ask_operator`.** A Secret ask (ADR 0109) reads no
vault and hands the chat no value. The person answers it in purlis's window, filling or
declining it. A harness prompt in front of it would ask the person twice, as it did before
`ask_operator`.

- **Only the tool.** `purlis secret ask` through the shell has no rule and still asks, as
  `secret exec` does.
- **A grade's chat is handed none of it** (SK-16), like every own-scope tool.
- **An operator's or a project's `ask` or `deny` still wins**, as Claude Code merges them.
- **Policy still refuses** an ask for a key the persona doesn't declare where
  `sandbox.undeclared-secret-asks` is `false` (#5078). The ask is answered `declined: policy`
  before it reaches the person.
