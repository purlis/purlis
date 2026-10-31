# Amended 2026-10-27: a grade's dollar limit is priced from its model, or the dearest known

Ticket #3469, the operator's ruling G12 of 2026-10-27 on spec #2762. It replaces the cost bound of
the amendment of 2026-10-26 (§4), which priced every grade at the dearest token any model charges.

**A grade has a dollar limit**: its token cap at the price of the model its chats run on. A
model's price is the dearest a token of any kind costs on it, its output at the dearer of its
context tiers and of fast mode, so a grade whose chats report their cost never reaches the limit
before the cap. Where the model is not named, or its price is not known, the limit is the cap at
the dearest known price (75 dollars a million, Claude Opus 4 and 4.1's output), so it is never
set below what the cap's tokens may cost.

**The model is the harness adapter's word.** Each adapter reads which model a grade's chat would
run on from its own words (`grade_model`), with nothing in the neutral core: Claude Code's is the
last `--model` in the profile's command, then `ANTHROPIC_MODEL`, with a family alias (`opus`,
`sonnet`, `haiku`) read through the variable that remaps it (`ANTHROPIC_DEFAULT_OPUS_MODEL` and
its kin); none where Claude Code would pick. A `model` in Claude Code's settings files is not read:
the person's and the project's both reach a grade's chat, and a project's outranks the person's,
so reading one alone could price the limit below the model that runs, and the dearest is used
instead. Until SK-60 (#3240) lets the person
pick a grade's model, that is the model a grade runs on.

**Prices are a dated table in purlis** (`purlis_core::skills::grades::price`), read from
Anthropic's published prices, never fetched at grade time: a grade is planned and held without a
call out. A model that is not in it (a newer one, a cloud provider's id, priced by that provider)
is priced at the dearest, which only ever widens a limit. A newer dearest price raises the
dearest in the same change.

**The plan says the limit and the price it used** before Grade is pressed: the model's, naming it,
or the dearest, saying why. `purlis skill grade` knows no profile and says the limit at the
dearest.

**What the limit counts**: what each chat cost, where its harness said; and, for a chat that said
its tokens and not its cost (a cost not known, as #3273 words it), its tokens at the dearest price,
so the limit still bounds it. The host ends a chat whose cost passes what is left, as at the cap.

**A grade that reaches the limit stops at it, and its record says so**: `report.ended` is
`over-limit` when the limit is reached before the cap (`over-cap` where both are reached at once),
and the record keeps the price (`priced`) and, where a chat said no cost, what the limit counted
(`counted_usd`). A row that says `over-limit` with no limit, or a price past 1,000 dollars a
million (well past the dearest, so a row a newer purlis priced is still read), is skipped.
