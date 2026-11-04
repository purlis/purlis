### Added

- **A vault key can read another entry's value: a Linked entry.** When a chat asks you for a
  secret, choose **Use an entry I have** and pick one of your entries from a list grouped by vault,
  with a filter on top. The key reads that entry's current value every time, so the value is
  never copied and rotating the entry reaches every key linked to it. The chat hears that the key
  was filled and its name, never which entry it reads. The vault view shows a link under its name,
  `purlis vault verify` reports a link whose entry is gone, and deleting an entry says how many
  links will break (#5069, #5073).
