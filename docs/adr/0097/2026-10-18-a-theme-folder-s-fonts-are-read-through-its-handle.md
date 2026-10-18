# Noted 2026-10-18: a theme folder's fonts are read through its handle (purlis#2555)

§3's third crossing, "a reference to a file", is held by the folder's handle as well as its
path.

- **A level's font files are read through the handle `theme.json` was read through.** For a
  theme folder of a level (the person's, the project's, a workspace's), `fonts/` is opened from
  the handle the folder's `theme.json` and icon set were read through, refusing a link, and each
  face from the handle of `fonts/`, refusing a link again. When purlis serves the face to the
  window it reads it again from the same handles, under the same cap and digest.
- **The person's own `theme.json`** in their config folder, and a folder named to
  `purlis theme check`, are read by path; their fonts are read from a handle of the folder
  opened when the folder is read (for the config folder, once `theme.json` has been read).
- **A project's and a workspace's faces are drawn only as approved**, by the amendment of
  [ADR 0098](../0098/2026-10-18-a-project-s-font-files-are-approved-with-the-project.md) of the
  same day.
- **Off macOS and Linux no face is read**, as no theme folder is.
