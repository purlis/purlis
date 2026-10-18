# Noted 2026-10-18: a theme folder's fonts are read through its handle (purlis#2555)

§3's third crossing, "a reference to a file", is held by the folder's handle as well as its
path.

- **Every file a theme folder brings is read through the folder's handle.** `theme.json` and the
  icon set already were (#2323). A face is too: `fonts/` is opened from the handle the folder's
  `theme.json` was read through, refusing a link, and each face is opened from the handle of
  `fonts/`, refusing a link again. When purlis serves the face to the window, it reads it again
  from the same handles, still under the cap and the digest.
- **A project's and a workspace's fonts need the project opened.** A theme folder at the plane
  root or in a workspace brings its fonts only in a project the window opened through the trust
  ask ([ADR 0035](../0035-a-plane-is-untrusted-until-the-operator-opens-it.md)):
  `purlis_core::theme::Folders::opened` says so, and `Folders::of` reads the person's own
  folders' fonts alone. A theme that names a font where its fonts are not read is refused, and
  the complaint says why.
