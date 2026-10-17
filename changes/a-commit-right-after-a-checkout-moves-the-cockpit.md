### Fixed

- **A commit made right after a checkout in the cockpit's branch now shows.** `git checkout -b
  topic/one && git commit` in the branch the sidebar is focused on moved only the new branch's
  ref, which purlis began watching a moment after the checkout, so the cockpit kept its old count
  until something else moved. Once the new ref is watched, the window reads the branch again
  (#2463).
