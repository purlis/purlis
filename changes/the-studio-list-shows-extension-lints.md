### Added

- **The Skill studio's list and Needs you show what your skill lint extensions say of the
  project's own skills.** An extension that declares `skill-lint` used to speak only in a skill's
  Text tab; now its findings are on the skill's Library row and under Problems in a skill, and a
  persona's Skills line counts them the same way. The list asks each extension about a skill's
  files once and keeps the answer while the app runs, asks new questions for at most 100 ms per
  list, and shows nothing from an extension that did not answer in time, asking it again after
  30 seconds. Opening a chat asks nothing (#3940).
