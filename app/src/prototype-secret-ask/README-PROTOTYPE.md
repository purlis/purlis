# PROTOTYPE: the Secret ask's body (spec #5054, ADR 0109, #5070)

Throwaway. It lives on branch `prototype/secret-ask` and never merges to main. The operator reviews
it on #5070 before the window ticket (#5080) starts.

## The question

What does a Secret ask look like inside the ask surfaces #4561 settled (the pinned strip above the
input, the top-tab quiz card, the Inbox card), in every state the spec names? One body, drawn the
same in all three hosts.

## Open it

One command, from any clone of purlis, with nothing to install or build:

```bash
git fetch origin prototype/secret-ask && git show FETCH_HEAD:app/src/prototype-secret-ask/PROTOTYPE-artifact.html > "${TMPDIR:-/tmp}/secret-ask.html" && open "${TMPDIR:-/tmp}/secret-ask.html"
```

The dashed bar at the bottom is the prototype's, not the design. `[` and `]` go through the states,
`\` lists them, and the bar switches the surface (Strip, Quiz, Inbox) and Light or Dark. In "Arrives
while you type", click the terminal line, type, and press **Ask arrives**. The real controls work:
type a value, pick an entry, Change the key or vault, Store, Link, Not now. Stand-in data, in
memory; nothing calls the core.

URL: `?s=<state>&at=strip|quiz|inbox`, `&closed` folds the strip, `&bare` hides the bar.

## The states

| # | State | Stories |
|---|---|---|
| 01 | New value, raised by purlis (a brokered command missed the key), declared | 1, 13, 15, 22 |
| 02 | Pick an existing entry → Linked entry | 23, 24 |
| 03 | Pick an existing entry, nothing picked yet | 23 |
| 04 | Rename the key and choose the vault | 25, 26 |
| 05 | Declared: the persona's purpose, then the chat's words, each marked as whose | 14, 15 |
| 06 | Not declared by this persona: a quiet tag, no extra step | 5, 14, 16 |
| 07 | 3 waiting on one key: one ask, one ✋ | 20 |
| 08 | Replace: "the chat says the current value doesn't work: …" | 11, 17 |
| 09 | Replace a Linked entry: names the entry it reads and how many keys link to it | 17, 18 |
| 10 | Broken link: names the gone target | 19 |
| 11 | Runner can't receive: store it here anyway; the chat hears declined | 47, 49, 50 |
| 12 | No vault yet: the first fill makes it | 27 |
| 13 | Not now: one quiet line | 30, 32, 33 |
| 14 | Filled, undeclared: the offer to add a Secret need, as a proposed change | 31, 43 |
| 15 | Arrives while you type: never takes focus | 21 |

`screenshots/NN-<state>-<surface>.png` draws each state on each surface, in dark; `light-*` a few
in light; `strip-folded.png` the strip before Fill; `masked-typed.png` the box with a value typed;
`focus-*-after.png` after an ask arrived mid-typing. `capture.mjs` also checks that an ask arriving
leaves focus and the typed line alone, and that the box is masked (all pass).

## Decisions for the operator

1. **The strip folds.** The strip shows one line (`Waiting on you · secret`, the headline, `n
   waiting`) and opens the body on **Fill**; the quiz and the Inbox card show the body at once.
   Should the strip open by itself, as here in its screenshots, or stay folded until asked?
2. **Who says what.** The persona's committed purpose is plain text under "<persona> persona"; the
   chat's purpose is italic, quoted, under "<chat> says", with the quote rule #4561 uses for a
   chat's note. Enough to tell purlis's words from the chat's?
3. **The undeclared tag** is a neutral badge beside the key, with a tooltip. Quiet enough, or too
   quiet?
4. **Change** opens the key and vault in place; a new name says the chat hears it. Or should
   rename and vault live behind a disclosure under the fill?
5. **Use an entry I have** is one filtered select of `vault/KEY` over every vault, with a line
   saying the key becomes a link. Grouped by vault instead?
6. **Runner can't receive**: the body stays fillable ("store it here anyway"), and the reason
   says the chat will hear declined. Or should the primary action read differently here?

## Rebuild

From `app/` (with its `node_modules`):

```bash
SA_OUT=$TMPDIR/sa-dist npx vite build --configLoader runner --config src/prototype-secret-ask/vite.standalone.config.ts
cp $TMPDIR/sa-dist/standalone.html src/prototype-secret-ask/PROTOTYPE-artifact.html
node src/prototype-secret-ask/capture.mjs
```

In a sandboxed chat, set `SA_CHROMIUM` to an installed headless Chromium and
`SA_ARGS="--single-process --no-zygote"`.

## Verdict

Waiting on the operator's review (#5070).
