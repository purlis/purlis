### Added

- **`purlis policy check` says what this version reads in a policy file, before you roll it
  out.** It prints the lowest purlis version that reads the file, each key this version doesn't
  enforce or doesn't know, and whether chats that reach a model would be refused under it, one
  line per fact. With no argument it judges `/etc/purlis/policy.json` as purlis reads it; with a
  path it checks a draft without judging its owner and permissions, and says so. It exits 0 when
  every key is read and enforced, 1 when one isn't, or `min-version` is above this version, and 2
  when the file can't be read. Until every install that reads a policy file is 1c or later, the
  file should carry neither `min-version` nor `models`.
