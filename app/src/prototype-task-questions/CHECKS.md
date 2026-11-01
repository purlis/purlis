# The prototype measured against the spec

`capture.mjs` ran these against the built page in headless Chromium at 1280×800. Its results are in
`checks.json`: 28 of 29 pass. The one failure is Foot, on purpose: it is the evidence for choosing Band.

## Visibility rule

"Something is shown only if the person can act on it or it answers 'what's happening?', in one place
at the level it needs."

| Level | Spec's place | Holds? | Measured by |
|---|---|---|---|
| Needs you | ✋, red, quiz, Inbox | Yes | ✋ equals what waits, and each item has one Inbox card, in all 47 window states |
| In progress | The task's row only | Yes, with one addition | No ✋, quiz, Inbox card or total above. The addition: opening the task shows the question, because story 16 and 30 say you can answer at once |
| History | The trail, on request | Yes | No Update for a question an agent handled; the trail opens only from What happened |

Three places bend the rule. Each is a decision below: the strip repeats a permission Notice (3), a
folded row says `1 asking` (6), and an agent's answer stays on screen after it closes (5).

## Smoothness checks

| Check | Verdict | Evidence |
|---|---|---|
| Nothing new to learn | Holds, one new key to confirm | The quiz wears the Inbox's card and the answers its buttons. F6 and Esc are the window's own keys. Say otherwise is new |
| No longer wait than today | Not measurable here | A prototype has no real agent. By design: the task's own tab can answer at once, and the 100 seconds is the core's |
| No stranded questions | Holds | One Inbox card for each item in all 47 states, including the top tab closed, a failed redirect and a lost connection |
| One item everywhere | Holds | The same words in quiz, strip and Inbox card; one answer clears quiz, ✋, card, row and tab count |
| Never steals focus | Holds in Chromium | A quiz arrived mid-word and typing carried on: `pnpm te` then `st` gave `pnpm test`. The real `AnswerQuestion` focuses its box on mount, so the build must not reuse it unchanged |
| A collapsed task still shows its state | Holds | A folded row keeps its count and says `1 asking` |
| Today's flows keep working | Partly | Permission buttons and the browser takeover stay as they are. The real Inbox, tabs and terminal were not exercised |

Also measured: the pause lasts about a second, a double click during it lands on nothing, and it
keeps the quiz's height (162px and 164px). What you typed survives an agent's answer. Every quiz says
why it reached you. No state scrolls sideways, and no state threw a script error.

## Not checked

WebKit, which is what the app draws in on macOS. Real xterm reflow: the quiz takes 170px from the
terminal, so a real pane resizes and the harness redraws. Axe and contrast. `tsc` and lint were not
run, to spare a loaded machine. Remote and cluster runners, which the spec itself lists as unverified.

## Decisions for the person

I made each of these to draw the page. All are open to your ruling.

1. **The strip and quiz stand in the pane's band, at the top.** A chat's input is inside the
   terminal, and nothing may draw over the terminal, so "directly above the input" can't be drawn.
   Band is literally above it, with the output between. Foot sits by your eye, but the input line
   jumps 170px when a quiz lands (`foot-*.png`); in Band it moves 0px. Band costs your eyes a trip to
   the top of the pane.
2. **A task's quiz replaces its strip.** In a task's tab the quiz already holds the same words, so
   drawing both would say it twice. The strip stays for everything else.
3. **A permission ask shows the strip and its Notice together**, as stories 46 and 47 ask, and they
   say the same thing one above the other (`06-strip-permission.png`). Dropping the strip while a
   Notice is on screen would hold the visibility rule better, but it amends story 46.
4. **The count is per top tab.** `2 more` counts that tab's questions; other tabs' wait on their own
   tab count and ✋. The spec doesn't say which it means.
5. **Say otherwise is a small addition** to the closing line. Story 37 needs a way to give your later
   word, and Go to task alone would leave you to type into a terminal. The line has to hold the
   agent's answer until dismissed, which sits close to "clears on answer"; it should be masked and
   not saved.
6. **A folded row says `1 asking`.** The spec says in-progress is on the task's row only, and also
   that a collapsed task still shows its state. I took the second.
7. **The trail has no words.** The audit is metadata only (ADR 0075), so a closed question can't be
   reread from its trail; the task's own chat still has it. The trail lives in the task's Activity,
   where a question's line is today, and opens from What happened.
8. **Time reads `now`, then `1m`, `2m`**, the Inbox's own wording, and it runs from when the question
   reached you, not when it was asked. There is no countdown of the 100 seconds, because nothing can
   be done with it.
9. **There is no Later on a quiz.** The spec has none. To skip one, you use the Inbox.
10. **The setting is drawn at Settings › Chats**, a stand-in place. The first-time line's answers are
    Answer from each session and Keep this; answering your first quiz also counts as keeping it.
    "Session" there is a word the person may want to check against ADR 0072's nouns.
