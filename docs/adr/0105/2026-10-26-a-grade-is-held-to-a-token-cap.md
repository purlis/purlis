# Amended 2026-10-26: a grade is held to a token cap the person sets

The operator's ruling of 2026-10-26 on #3229, which also confirmed §3's send (ADR 0061, amended
2026-10-25). §4 said the cost comes first and the person's account pays. A grade is now also held
to a **token cap**.

- **The person sets it, on this machine**: `[skill_grades] token_cap` in `charter.local.toml`,
  200,000 tokens where it is not set, from 1,000 to 100,000,000. It is this machine's alone,
  because the person who pays sets it; in the committed file it is refused. The plan says it
  before Grade is pressed, and sets it in place.
- **Each chat is told what is left.** purlis reads what the chat has spent at each look. A chat
  that passes what is left is ended, and is no run: it was cut short before it could say what it
  would have started. No more chats are asked for. A chat that ends below the cap but takes the
  grade to it ends the grade too.
- **The end says it**: that the grade stopped at its cap, after how many chats, and what they
  spent. The report and the record keep the cap and the tokens (`cap`, `spent_tokens`).
- **A cap that cannot be held stops the grade.** A chat whose program says nothing of what it
  spent leaves the cap unheld, so the grade stops after that chat (`spend-not-reported`) and says
  so. Today only Claude Code reports a chat's spend (`usage::record_spend`). Grading on another
  program waits until its spend is read (#3272).
