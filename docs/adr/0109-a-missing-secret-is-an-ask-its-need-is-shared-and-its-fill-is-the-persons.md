# A missing secret is an ask: its need is shared and its fill is the person's

**Accepted 2026-11-03**, from the operator's grill of the secret ask (M61 · Sandbox experience).
It builds on #1407 (brokered `purlis secret exec`, persona tools), #3186 (the vault entry picker)
and ADR 0107 (where a task's asks are answered).
**Amended by ADR 0111** (2026-11-05): a Secret need is written only when the person asks, in the
Fill notice, and a person may allow an undeclared program there.

## Where purlis was

A persona names one vault in its committed definition, and every teammate's chat looks for a
vault of that name; the values live in each person's own keyring (ADR 0047). When a key was not
there, `purlis secret exec` failed with `SecretNotFound` and the chat stopped and told the person
to go and add it. The Ask model already had a kind that elicits a secret, and a power to answer
one (`AnswerASecret`), but nothing raised it for a vault. The operator, 2026-11-03: *"when
persona need secret - it can automatically in purlis UI open some form/modal/inline and user can
choose existing secret from some vault - and or add new secret. After we need to smartly keep in
persona local/persistant memory/prompt"*, and *"agent should always know that can ask any
secret, so even if user asking something that not documented … it can initiate interactive
secret requesting flow"*.

## The decision

- **A missing secret is a Secret ask, never a dead end.** purlis raises one when a command it
  hands secrets to (`secret exec`, a persona tool, a browser login, a profile's credential) finds
  its key missing. The chat may also raise one at any time, for any credential, declared or not,
  with `secret_ask(key, purpose, vault?)` or `purlis secret ask`. Every chat is told it can, in
  its session-start line, so every harness knows, not only one with purlis's tools loaded.
- **Only the person, in the window, answers it.** It is never answered by an asking chat, never
  passed on, never routed by the "where you answer" setting, and never answered from a runner or a
  remote client. It shows where every ask that needs the person shows (ADR 0107's surfaces).
- **The fill is the person's and stays on their machine.** A new value is written into their copy
  of the persona's vault; picking an existing entry writes a **Linked entry**, so the value is
  never copied. A missing vault is made on the first fill, on the default provider. Every fill is
  kept in a vault: there is no "this chat only" secret.
- **The need is shared.** A **Secret need** (key and a one-line purpose) is committed with the
  persona, so the next teammate's ask arrives named and explained. purlis offers it after a first
  fill as a proposed change the person approves; it never writes it on its own, and nothing about
  a secret, value or fill, is written to memory or a prompt.
- **The chat hears filled or declined, and the key's final name.** Never a value. The person may
  rename the key or pick another vault before filling. A brokered command waits on the ask until
  just before its harness's command timeout, then exits saying it waits on the person, and the
  chat is nudged when the ask is answered. A decline is said as a decline, and the same chat is
  not asked about that key again.

## Why

- **Not persona memory or the prompt.** A persona is shared. Its memory reaches every teammate
  and would hold one person's vault layout, which means nothing on another machine, and puts a
  secret's whereabouts where the model reads it. The keyring is already per person, so the fill
  is remembered by being stored, with no second record.
- **Any secret, not only declared ones.** Limiting asks to declared needs would bring the dead end
  back for every new piece of work, which is where the person most needs the flow. The cost, an
  ask the chat was steered into by something it read, is met where a value can go: the sandbox
  limits the hosts a command reaches, the ask quotes the chat's purpose as the chat's words, and
  an undeclared key is shown as one, with no extra step.
- **A link, not a copy.** A copied value goes stale on rotation in one place and not another; a
  Linked entry keeps one value and makes a broken link visible, in the next ask and in
  `vault verify`.

## Considered and not taken

- **Keep the fill in persona memory or `persona.md`**: shared, and wrong on every other machine.
- **A per-chat secret held in memory only**, like a chat-level sandbox grant: a second, weaker
  store beside the vault.
- **Asks only for declared needs**: brings back the dead end the decision removes.
- **Block until answered with no limit**: the harness kills the command and the chat is stranded.
