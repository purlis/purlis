### Fixed

- **An extension that is grading says so** (Skill studio preview, #3236). Anything else it is asked
  during a grade is refused at once with that reason, rather than waiting and failing. A Stop ends
  a Grader's question promptly while purlis is writing to it. A Grader whose id is over 64 bytes is
  refused as it is declared, so none of its grades is lost from the record.
