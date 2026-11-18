### Added

- **Issue and pull request numbers in a pane are links.** `#6223` opens that issue or pull
  request of the repo the chat's folder is a clone of, on its GitHub or GitLab, in your browser.
  `owner/repo#6223` opens that repo's on the same forge without asking when the repo is one of
  the folder's remotes, and asks first for any other repo. Words like
  `docs/guide.md#12` are the file where that file is in the folder, and an issue's otherwise
  (`vercel/next.js#123`). A marked link whose words are `owner/repo#6223` asks first unless it leads to that issue or pull request over https; with any character outside ASCII in them (a look-alike slash, hyphen, digit or letter) it asks over any page, and a `.git` after the repo is read as the repo. Where the folder has no remote on a forge purlis knows, or
  several and no `origin` among them, the number stays plain text (#6223).
