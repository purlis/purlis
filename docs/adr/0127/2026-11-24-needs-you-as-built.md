# Amendment, 2026-11-24: needs-you as built (M75 slice 19, #7028)

The decision and spec §6.8 and §8.3 to §8.5 say an ask reaches the person only if the chat is
attended and a window can be reached; otherwise the chat is refused and one needs-you item is
raised. Slice 19 built it, and settled what the text left open. Nothing here reverses a ruling.

- **One item is a rule, a resource, a workspace and a request digest** (review round 2, S2-SF4).
  The digest is of the action, the resource and the request as the host made it. An Allow's grant
  names the digest, so it covers the request the item showed and no other.
- **The caps' three limits read like this.** `asks.needs-you-max-items` keeps the most recently
  raised; `asks.needs-you-max-days` lets an item go that long after its first raise;
  `asks.needs-you-quiet-days` lets it go when nobody raised it again for that long. The spec named
  the numbers (50, 30, 7) and not which clock each reads.
- **The wait's cap is `Deadline::below_hook_timeout(HOOK_TIMEOUT)`**, 58 s for Claude Code. An
  unanswered wait is refused and raised (`why: no_answer`), never left to the harness's pane.
- **Only a Once grant is deferred**, and it is bound to a chat and a request digest, spent once,
  within 30 minutes of the answer. It lives in memory, as a lapsed ask's pre-answer does
  (`harness::hooked::lapsed`): a restart drops it, and the chat asks again. Every wider grant is
  slice 20's, in the sealed answers.
- **Allow takes the person's press, and `purlis inbox` has none to pass.** The headless command
  has `list`, `deny` and `dismiss`, and the type that answers an item allows only through a
  `Press`, which clippy and `the_press_is_made_only_by_the_windows_command_layer` keep to the
  window. So the rule that a headless machine never allows is held by the types, not by a check.
- **The rule offer after two Allows is gone** (Q314.15): the core's `askrules::offer`, the host's
  `RuleOffered` and the window's offer row. Rules made before stay in Rules I made until slice 20's
  Allowed list replaces them. The cool-off stays; its three numbers are `asks.cool-off-*`, which
  the host reads in a later slice.
