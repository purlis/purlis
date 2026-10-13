### Security

- **A spooled tool line keeps no digest of its arguments.** When the app does not take a hook's
  line in time, the line is kept in the chat's spool until the next drain. A tool line's
  arguments digest is now left out of it, so nothing on disk holds a digest that could be
  matched against a guess at the arguments. The event a drained line becomes has no `args`
  (#977).
- **Session credentials are written through the folder that was checked.** Each scope's
  credential file is made and renamed inside the directory purlis checked, through the
  descriptor it checked it on, so a folder put in its place afterwards is never written to
  (#992).

### Fixed

- **The inside-a-chat check finds an app listening in its fallback folder.** A project whose
  path is too long for a socket beside it has its app listen in a per-user fallback folder; the
  check now finds that app too, as it finds one listening beside the project.
- **A hook socket under a folder with a newline, extra spaces or letters past ASCII is read
  right.** The host check read such a path as another, and refused a real host (#1146).
