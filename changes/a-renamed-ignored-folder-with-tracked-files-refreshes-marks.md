### Fixed

- **Renaming an ignored folder that holds a file you added with `git add -f` updates a branch's
  change marks.** The rename was taken for a change to ignored files only, so the explorer's
  marks waited for the next full read (#874).
