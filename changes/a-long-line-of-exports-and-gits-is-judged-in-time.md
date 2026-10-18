### Fixed

- **A long line that alternates exports or assignments with git commands is judged in time
  instead of refused for taking too long.** The project-root, clone update and project-root
  update guards gave every git on a line its own copy of everything the line had exported and
  assigned before it, so such a line cost the square of its length: at the size cap one took
  several seconds, past the guard's two-second budget, and was refused. The gits on a line now
  share that environment, the verdicts are the same, and the time grows with the line (#2648).
