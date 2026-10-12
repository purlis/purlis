### Changed

- **A report is filed with purlis's own sign-in, never `gh`.** A sandbox block's **File report**
  searches and files on purlis's tracker as you, with the GitHub account you signed in to in
  Settings › You › Accounts, and no longer asks `gh` under whatever login it holds. Without that
  sign-in nothing is sent, and the link that files the draft in your browser is shown instead.
  `purlis report` from a terminal shows the draft and that link: a command holds no sign-in
  (#806).
