### Changed

- **The Skill studio keeps its own records in purlis's data folder, outside the project.** Drafts,
  tries, this machine's grades, grades under way, grading after each edit, the kept grade chats
  and the records of which workspace and persona folders reach chats now live in
  `<data>/app-state/<clone key>/`, which a chat's sandbox reads and never writes. The first time
  you open a project after updating, purlis moves your drafts and this machine's grades there.
  Grading after each edit is off again until you turn it on from a grade's plan, and where a
  sandboxed chat had run in the project, each workspace and persona folder waits for your Accept
  in the Skill studio again before its skills reach chats (#3718).
