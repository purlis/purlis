### Fixed

- **A forge sign-in's renewal no longer gets stuck behind a stale marker, and signing in or out waits for a renewal in progress.** The note a process leaves when it holds a renewed token it could not save now ends after 30 minutes and is cleared by signing in again; a note that cannot be written or read stops the renewal instead of letting it go ahead; and signing in or out takes the same lock as a renewal. A `CI_JOB_TOKEN` is accepted outside the usual token shapes only when it is a `glcbt-` token, a JWT or GitLab's original 20-character token (#7297).
