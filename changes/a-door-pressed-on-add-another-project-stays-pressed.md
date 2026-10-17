### Fixed

- **A door pressed on Add another project stays pressed.** On a machine that has been through
  the setup steps, the first `+` of a launch drew the setup steps' welcome for a moment and then
  swapped it for the plain one. A door pressed in that moment, such as Create a project, was
  undone and the welcome came back. purlis now finds out where you are in the steps before it
  draws the welcome, so the right one is drawn the first time (#2400).
