### Changed

- **An interrupted grade's spend gives the next grade's estimate.** A grade's marker now
  records how many chats the grade asked for so far, whether they ran or not. When purlis
  stops during a grade, its row still says what those chats spent. The next plan shares that
  spend over those chats and says the estimate comes from an interrupted grade. Before this, a
  skill whose only grades were interrupted showed "ran no chat" in its plan. An interrupted
  grade recorded before this version still gives no estimate (#3602).
