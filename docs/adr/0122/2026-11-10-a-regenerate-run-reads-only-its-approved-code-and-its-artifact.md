# A Regenerate run reads, of the plane, only its approved code and its artifact

**2026-11-10, spec #5948, from the second adversarial review of #5978 (finding 2).** Amends §6
of ADR 0122 and the amendments of 2026-11-09 (running Regenerate) and 2026-11-10 (every word of
the command approved).

Regenerate's approval fingerprints the code folder (the script's skill's folder, else its own)
and every word of the command. A run's sandbox read what a chat reads: everything but the
plane's denial classes. So code the approval never saw still loaded: an import of
`../lib/q.mjs`, a `node_modules` above the folder, a second file named in the command, a link
in the folder to a file elsewhere in the plane (fingerprinted by where it points, not by what
it holds). A teammate who edits `../lib/q.mjs` had the next Regenerate run it, unapproved, and
it could copy any readable file of the plane into the data the page shows.

- **A run reads nothing of the plane but its approved code folder and its artifact's own
  folder** (`purlis_core::artifact::regenerate::plane_reads`). Its profile gains, for each name
  of the plane's root, a denial of `file-read-data` below it except under those two folders.
  System paths and everything outside the plane read as before.
- **What is outside the approval cannot load, so hashing more is not needed.** A file outside
  the code folder in the plane, whether reached by `..`, by a link or as a word of the command,
  is refused when opened, and a run that needs one fails and leaves the data as it was. So link
  targets and other plane files in the command are not fingerprinted: they cannot run.
- **What a file is stays readable** (`file-read-metadata`), so an interpreter resolving a path
  through the plane's folders works; the working folder's own entries are listed, never its
  files read. Nothing is given back inside the two folders, so a denial class there still holds.
- **One rule, both platforms.** It is a rule of the Seatbelt profile, which Linux's supervisor
  reads and judges every open by (`purlis_linux_sandbox::profile`), so macOS and Linux hold a
  run to the same text. A script that read workspace files outside its folder for its data must
  now keep them in its code folder or the artifact's.
- **What is approved is unchanged**: the code folder's fingerprint and every word of the
  command. The prompt names that folder, and that folder is now all of the plane that a run reads
  besides its artifact.
