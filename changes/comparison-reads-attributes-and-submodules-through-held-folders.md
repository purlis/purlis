### Fixed

- **A submodule checked out at another commit shows as changed.** A comparison with what is not
  committed now draws a submodule whose folder checked out a commit its pointer does not name
  as git does, the commit it checked out on the working tree's side. purlis reads that commit
  from the submodule's git folder as plain files and starts no git in it (#704).

### Security

- **A comparison reads attribute files only through the branch's own folder.** The
  `.gitattributes` files that decide whether a file is binary and how its line ends compare are
  now opened from the branch's folder one component at a time, following no link, as the files
  themselves are. A `.gitattributes` that is a link, or one in a folder swapped for a link, is
  not read.
