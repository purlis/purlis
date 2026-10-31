# Amendment, 2026-10-30 (#3291): one signal per refusal

The field family's error line (2026-10-22, "the field family") was an assertive live region that
also took `role="alert"` while it said something. That reads as two signals in one render (an
alert and a live-region change, heard twice with Chromium and NVDA), and when one send refused
several fields, every line spoke at once and cut the others off, and the typing echo with them.
The rule is now the common practice for form validation (APG, WCAG 3.3.1 and 4.1.3):

- **A field's error line is polite, never an alert.** It is a polite live region from the start,
  empty and in the accessibility tree, so its words are heard when they land (VoiceOver on WebKit,
  #3033) and wait for the typing echo. Saying something, it is the box's description
  (`aria-describedby`) and the box is `aria-invalid`, so the error is reachable from its field.
- **A refused send is one alert summary.** A `Form` whose own checks refuse, or that is handed
  the core's refusal, says it once, in one `role="alert"` for a screen reader alone: one field is
  its name and why ("Name: A name is letters…"), more are how many and which ("2 fields to fix:
  Name, Port."), after what the core said of the whole. The form's owner hands it `refused` with
  `answer`, a count of the core's answers, so each new answer is said once and the person's edits
  to it are not a new one; what draws the fields inside a form calls `useFormRefusal()` in the
  handler that gives them their words. The fields' lines are quiet (`aria-live="off"`) while the
  summary shows, so they do not speak over it; the person's next change ends it (a native change,
  or a select's or combobox's pick) and the lines are heard field by field again. A refusal again
  says the summary anew, even in the same words. The focus still goes to the first field refused.
- **One place says it.** A refusal said elsewhere as its own alert is not said again by the form:
  a collection's Rename leaves out the reasons its entry says under itself. A form that is not
  inside a dialog's form (the git identity's on a Notice) is a `Form` of its own, so its rows'
  refusals are its one summary, not an alert a row.
- **A settings row in a form follows it** (`SettingRow`): its refusal line is a polite live region
  quiet under the summary. A settings row outside a form is its write's one save, and its refusal
  stays that save's one alert.
- **Elsewhere, a refusal is still an alert**: a dialog's or a view's trouble line is the one
  signal of what it refused.

The operator's VoiceOver pass, hearing a refusal exactly once, is the check that needs hardware.
