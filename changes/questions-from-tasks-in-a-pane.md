### Fixed

- **A question from a task keeps its line breaks.** In a chat's pane, a question a task wrote
  over several lines showed each line break as `⟦U+000A⟧` on one line. It is now drawn over the
  lines the task wrote (#5213).
- **Questions from tasks sit under the pane's head, in its notices band, and close.** They were
  drawn above the head and took up to half the pane with nothing to close them. Each question
  now has a Close, which puts it away from that pane only: it stays in the Inbox and on ✋ until
  you answer it. The band's Minimise folds the note about where questions go into the bell,
  and its buttons no longer sit over the question's title (#5213).

### Changed

- **The note about where questions from tasks go is one line.** "Questions from this chat's
  tasks now come here" has **Change**, which opens Settings at Where you answer, and a × that
  puts it away for good. The "Answer from each session" and "Keep this" buttons are gone; keeping
  the setting needs no answer (#5213).
