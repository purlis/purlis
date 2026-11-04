### Added

- **Your later word overrides an agent's.** When a task's asking chat answers a question you had
  open, the quiz closes quietly and leaves one line saying who told the task what, with Go to
  task and Say otherwise. It stays until you dismiss it. What you say reaches the task as your
  word, marked as replacing the chat's answer, and the chat is told. Only Say otherwise
  replaces it: an answer you send from the quiz just after the chat's is not sent, so you see the
  chat's answer first. Anything you had typed in the quiz is kept in the box (#4566).
- **A short pause before the next quiz.** After you answer, the quiz's place says where your
  answer went for about a second before the next question appears, so a second click can't land
  on it (#4566).
- **A task's question the window can't draw** (a prompt in its own terminal) takes its place in
  the same queue at the top, as a line linking to the task's pane (#4566).

### Changed

- A tabbed chat's own question now counts the questions waiting behind it, and the line that
  offers where you answer shows in one pane only (#4566).

### Fixed

- After you answer a quiz, its box is cleared and Send is held, so a second Enter sends nothing
  (#4566).
