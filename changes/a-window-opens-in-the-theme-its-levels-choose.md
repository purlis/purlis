### Fixed

- **A window opens in the theme its levels choose** (#2602). A machine-default theme folder, a
  project's choice and a workspace's are drawn from the window's first frame, in the native
  window too, with no repaint once purlis has read the project.
- **A window opens in the high-contrast appearance when the system asks for more contrast**
  (#2582). With Increase contrast on (macOS) or a high-contrast desktop setting (Linux), a theme
  with an appearance tagged high contrast is drawn from the first frame.
- **An appearance a theme does not have honours more contrast**: the theme's first appearance
  for the system's contrast is drawn in its place.
