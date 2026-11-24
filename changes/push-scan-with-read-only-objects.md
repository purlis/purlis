### Fixed

- **A push from a task chat is scanned again.** The secrets scan read the push through git's
  merge-diff mode, which makes a temporary object directory inside the clone's `.git/objects`;
  a task chat's sandbox can't write there, so every push was refused with "unable to create
  temporary object directory". The scan now gives git a scratch object directory of its own,
  under the temporary folder, that reads the clone's objects, and removes it afterwards. What
  the scan reads is unchanged.
