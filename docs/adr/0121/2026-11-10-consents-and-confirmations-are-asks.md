# Amendment, 2026-11-10: consents and confirmations are asks, and where each is counted

Built for #6000 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 2, which names "a sandbox block or consent" and "a command's confirmation in the window"
as Blocking asks. Two choices that decision leaves open:

- **Which consent is an ask.** What a project expects and this machine has not allowed ("Allow on
  this machine", ADR 0093) is one: its sandboxed chats are refused it until the person answers.
  It is listed in Asks and stays a Notice too. Both are derived from the one state, so an answer
  in either place settles both. It is an ask only while there is something to decide: if policy
  locks it, or nothing it lists can be allowed on this machine, only its Notice says so. The
  sandbox's one-time offer to a project made before the sandbox existed stays a Notice. It
  decides a policy and stops no chat: the project's chats keep running as they did.
- **Which project counts a command's confirmation.** A confirmation belongs to the machine, not
  to a project, so it is counted once, under one project: the project the command runs in, if
  the window has it open. Otherwise it goes under the project in front of the focused window,
  and failing that, the first project the window holds. The command says the same project in its
  line ("waiting for the person: Asks (✋) in the purlis window, project smart-ide, for up to 2
  minutes"), or names none when it runs in no project.
- **Where a command's confirmation is answered: on its row in Asks, and nowhere else** (#6136).
  It had a modal dialog of its own as well, which covered the ✋ and the rest of the window, so
  the row it was counted on could not be reached while it was up. The dialog is gone, as
  decision 1 asks (nothing is listed in full in two places): the row shows the code the
  command's terminal shows, and its yes and its no (AskDecision) answer it. Its two
  minutes, taken back as a no, and its one question at a time are unchanged. A non-modal dialog
  was weighed and not taken. What the dialog guarded is held on the row already: a dialog
  arrives under the person's keyboard, so it started on Cancel and armed its yes only once
  nothing had been typed for a moment (ADR 0095, amended 2026-11-01).
  A row in Asks takes no focus when it arrives, so no typing lands on it, and its yes waits for
  the row to settle (ADR 0097), so no press meant for another row lands on it. The code is read
  on the row as it was in the dialog, against the terminal, and is a check on which command
  asks, not a secret. With no project open in the window there is no row to answer it, so the
  command is refused at once, and told to open one, rather than kept waiting two minutes.
