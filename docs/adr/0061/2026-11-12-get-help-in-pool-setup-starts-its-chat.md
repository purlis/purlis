# Amended 2026-11-12: Get help in Pool setup starts its chat on a message purlis sent

Ticket #4497 (spec #4469, stories 59 and 60: "Get help opens a chat holding the step and its
error, with no secret values", and "that chat changes nothing itself"). **Accepted 2026-11-12**
by the steward, under the person's standing go-ahead for delegated rulings, with the safeguards
below as built.

Pool setup's **Get help**, on a step that failed, is the third chat this record's machinery starts
on a sent message (`curation::start_sent`, `SentFor::PoolSetupHelp`). As for Fix, the person's
press is the moment they asked for help, so the chat works from its start, not on a prompt left
for them to send. And it can't be typed: it holds the step's findings, doctor's lines and the
error, which take more than one line.

- **Only the person's press starts it.** The command (`pool_setup_get_help`) is the window's
  alone; no chat, tool, hook or `purlis` word reaches it.
- **The message is purlis's own**, written by the core (`purlis_core::poolsetup::help`) from what
  Pool setup shows: the step, its findings, doctor's lines, its commands and the error the window
  showed, each quoted as data under purlis's sentence that says so. No sign-in, token, key or
  credential is read to write it, and any line that looks like one is left out.
- **It changes nothing itself.** The message asks for an explanation and says every fix is the
  person's, in Pool setup. That is the brief; the fence is the sandbox: the chat starts only under
  its project's sandbox and is refused otherwise, and there no chat writes purlis's config home,
  reads the pools' records or their kubeconfigs, or reaches Docker's socket, and `purlis pool`
  refuses every change from a chat.
- **Nothing is typed**, so nothing is gated by the board, as for Fix.

Every other prompt this record prepares is still typed and never sent. A fourth reason is a new
amendment here, and a new `SentFor`.
