### Added

- **A Grader extension can grade a skill, its chats run by purlis** (Skill studio preview,
  #3236). A Grader asks for each chat of a grade, and purlis runs it as a grade's chat, held to the
  chats, token cap and dollar limit the plan showed, and tells the Grader what it did, until the
  Grader answers its scores. A Grader that asks for more than the plan, or answers something
  purlis can't read, ends the grade with a sentence saying why, and what ran is still scored.
  Extensions speak protocol 4 for this.
