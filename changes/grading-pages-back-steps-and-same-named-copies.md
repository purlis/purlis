### Fixed

- **Back to the scorecard always lands on that grade's scorecard.** A saved chat opened from an
  old grade tab went back to whatever page came before it; it now opens its scorecard (#3978).
- **Leave it running and Back to its grades add no step.** Back on the Grades tab afterwards no
  longer reopens the running grade (#3978).
- **A scorecard or saved chat of a workspace's or a persona's skill opens from an old grade tab.**
  It used to open the skill's page instead; now it opens the scorecard, and the Skill studio
  moves to that workspace or persona (#3978).
- **A grade of a same-named copy at another level no longer moves this copy's Grades tab.** A
  workspace `deploy` ending its grade no longer raises a notice on the project's `deploy` (#3978).

### Changed

- **Every Skill studio page reads and fails alike.** While a read is under way it holds
  placeholder rows and says what it is reading after about a second; a read that failed says why
  in place, with Try again (#3978).
