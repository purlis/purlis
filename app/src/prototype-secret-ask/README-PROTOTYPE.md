# PROTOTYPE: the one Secret ask notice (#5227, spec #5235, ADR 0111; first drawn for #5070)

Throwaway. It lives on its own branch and never merges to main. The operator reviews it before the
window ticket (#5080) starts. Every name, vault and host is a stand-in, in memory; nothing calls the
core.

## The question

What does one notice for one run look like, carrying whichever of vault access, missing keys and a
program's allowance that run hit, inside the ask surfaces #4561 settled? ADR 0111 fixes the
behaviour, so this question is only about layout and words.

## Open it

From any clone of purlis, with nothing to install or build (run it yourself: a sandboxed chat can't
open a browser):

```bash
git fetch origin 5227-prototype-again-one-notice-fill-and-46agkdx3 && git show FETCH_HEAD:app/src/prototype-secret-ask/PROTOTYPE-artifact.html > "${TMPDIR:-/tmp}/secret-ask.html" && open "${TMPDIR:-/tmp}/secret-ask.html"
```

The dashed bar at the bottom is the prototype's, not the design. `[` and `]` go through the states,
`\` lists them, and the bar switches the surface (Strip, Quiz, Inbox) and Light or Dark. The real
controls work: type a value, pick an entry, Change, tick Remember, choose an Allow, answer.

URL: `?s=<state>&at=strip|quiz|inbox`, `&open` opens the strip (it stays folded until Fill), `&bare`
hides the bar.

## The states, and their pictures

`screenshots/<NN>-<state>-<dark|light>.png` draws each state as the top-tab quiz, in a tall window
so the whole notice shows. `one-notice-strip-folded-*`, `one-notice-strip-open-*` and
`one-notice-inbox-*` draw the full notice on the other surfaces; `one-notice-window-*` draws it in
an ordinary 1280×820 window, where the band scrolls under the action; `picker-open-dark` is the
entry picker, grouped by vault with a filter.

| # | State | Asked for |
|---|---|---|
| 01 | `one-notice`: vault access + one missing key + an undeclared program | the full one notice |
| 02 | `several-keys`: a field per key, one Allow | several missing keys |
| 03 | `scope-once` | Allow once, the default |
| 04 | `scope-chat` | Allow for this chat |
| 05 | `scope-always`: the committed line shown first, `--env`'s advice | Always allow |
| 06 | `scope-dont`: says the value is still stored | Don't allow |
| 07 | `no-program`: Fill only | an ask naming no program |
| 08 | `interpreter` | interpreter refused |
| 09 | `policy-lock` | policy lock |
| 10 | `tools-lock` | tools lock |
| 11 | `runner`: no Allow, **Store here** | runner chat |
| 12 | `remember-ticked`: the exact line, purpose editable, a `vault:` line | Remember ticked |
| 13 | `remember-unticked` | Remember unticked |
| 14 | `hostile-command`: controls drawn, one flow, nothing obeyed | added |
| 15 | `allow-only`: the key is held, the Allow alone | added (#5239) |
| 16 | `change-in-run`: a new name means no command to allow | added (question 2) |
| 17-18 | `answered-allowed`, `answered-dont`: the one quiet line | added |
| 19-30 | #5070's states in the same body (replace, broken link, picker, Change, no vault, 3 waiting, not now, arrives while typing) | #5080 |

## What it keeps from #5070's review

The strip folds until Fill. The persona's purpose is plain; the chat's words are italic and quoted
under "<chat> says". The undeclared badge stays, and the program gets the same one. Change happens in
place. The picker is the real `VaultEntryPicker`. A runner's action reads **Store here**. The
disabled action is no longer purple on purple, and the stand-in title bar no longer says "charter".

Remember uses #5240's words, from `SecretFill` and `SecretNeedLine`, with a row per key.

## Who says what

- **The app's** sit in the bordered **From purlis** panel: the program's path, the hosts, the vault
  line, T-4's caution or why there is no Allow. Nothing of the chat's reaches it, and the Allow's
  question names the path too.
- **The chat's** sit outside it, behind the quote rule: the command plain, monospace and wrapped, on
  one flow, with each control or invisible character drawn as a marked token (`ESC`, `LF`, `U+202E`);
  the purpose quoted and marked the same way.

## Open questions, with a recommendation

1. **Interpreter: when does this notice exist?** The brief wants the variant; spec #5235 and #5239
   say a program that runs what it is told is refused with no notice. *Recommend:* both. A **run** of
   `python3` stays refused with no notice, and this variant is only for `secret_ask --program python3`,
   where the chat is asking for a value anyway and Fill still has a use. The spec's test then reads
   "a refused run gives no notice".
2. **Change in a run.** The command names the key and vault, so a new name means the exact command
   can't be the one allowed. *Recommend,* as drawn: Change stays in place (the #5070 ruling), and a
   new name replaces the Allow with one line saying the next run asks again. "Use an entry I have"
   keeps the name, so it keeps the Allow.
3. **The vault's line is wider than the Allow beside it.** Allow once for curl would also let every
   chat opened as devops use the vault, on this machine, as the existing vault Allow does. *Recommend,*
   as drawn: one Allow, with the line saying so, and Don't allow grants neither. If that is too much
   for a once, the vault needs its own tick, which breaks "one Allow".
4. **A vault's line with no program Allow** (a locked notice, or a declared tool, that also lacks the
   vault's access) has no Allow to ride on. Not drawn. *Recommend:* the vault's own Allow as a button
   on that line, since nothing else can grant it.
5. **Runner: Always allow or none?** Spec #5235 says "Fill and Always allow still work"; the brief and
   #5243 show no Allow. *Recommend,* as drawn: none on the notice, with the chat told to declare a
   tool, and amend the spec's line, since Always allow's Run grant is what a runner can't have.
6. **Remember with several keys.** One tick writes a line per key, each purpose editable. *Recommend,*
   as drawn: one tick, because a person who wants fewer can clear this and use the vault row later.
7. **Not now in a combined notice.** Drawn as declining all of it: nothing stored, no grant. In
   `allow-only` there is no Not now, because Don't allow is the same answer. *Recommend:* keep both.
8. **The action's words.** `Fill and allow once`, `Fill, don't allow`, `Allow once` where there is
   nothing to fill; the reviewed `Store`, `Replace`, `Link`, `Store here` where there is no Allow.
   *Recommend:* keep them. #5240's real button says Fill, so #5080 should pick one word for the plain
   case.
9. **Height.** The full notice runs about 460px, and the band stops at 60% of the pane, so in an
   ordinary window it scrolls. Drawn with the action stuck to the foot, so it is always in reach.
   *Recommend:* keep that, and don't collapse the app's facts, since they are the reason to read it.

## Found while drawing, for #5080

- On the ask's card the theme's sunken field has no visible edge (`--border-control` is clear), so a
  value box or a purpose to edit reads as plain text. Today's `SecretFill` has this too. Drawn here
  with `--border-strong`; #5080 should settle it in the theme's parts.
- The old standalone config lost the app's aliases as main moved on; it now passes `resolve` and `css`
  through.

## Rebuild

From `app/`, one at a time:

```bash
npm ci --ignore-scripts
SA_OUT=$TMPDIR/sa-dist npx vite build --configLoader runner --config src/prototype-secret-ask/vite.standalone.config.ts
cp $TMPDIR/sa-dist/standalone.html src/prototype-secret-ask/PROTOTYPE-artifact.html
node src/prototype-secret-ask/capture.mjs
```

`capture.mjs` redraws `screenshots/` and checks focus, the fold, one field per key, the action's
default, the disabled label's contrast, no Allow where none may be given, and the hostile command.
In a sandboxed chat, set `SA_CHROMIUM` to an installed headless Chromium and
`SA_ARGS="--single-process --no-zygote"`. #5070's own pictures stay on `prototype/secret-ask`.

## Verdict

Waiting on the operator's review, before #5080.
