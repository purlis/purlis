### Added

- **A skill lint extension can answer the whole Skill studio list in one question.** An
  extension whose `contributes.skill-lint` says `"many-at-once": true` is asked
  `findings-of-each`: every skill of the list with its files, in one request, answered with one
  entry per skill in the order asked. Its confined program then starts once per list rather than
  once per skill, and gets the list's whole 200 ms instead of 100 ms per skill, so a lint that
  takes about 150 ms to start now shows its findings on the Library rows and in Needs you. One
  question holds at most 200 skills and 4 MiB of their files. An extension that does not say
  `many-at-once` is asked one skill at a time, as before, and the editor's live lint still asks
  `findings` (#4195).
