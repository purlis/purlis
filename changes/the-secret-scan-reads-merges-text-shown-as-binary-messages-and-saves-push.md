### Security

- **The secret scan reads more of what a commit or a push publishes.** A chat's push is now
  read for what a merge's own resolution adds (including a merge made with `git commit-tree`),
  for each commit's and annotated tag's message (credentials only, so a `Co-Authored-By` line is
  never refused), and, at commit and at push, for a text file git only shows as "Binary files
  differ" because of an attribute or a lowered `core.bigFileThreshold`. `purlis save` now scans
  what its own push would send before pushing, so a commit made by hand in the project is no
  longer published unscanned. Real binary files, LFS pointers, submodules, deletions and merges
  that add nothing of their own are let through as before (#1133).
