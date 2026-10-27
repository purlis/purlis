### Added

- **The event log records each browser's life.** It now says when a browser opens, when it is
  closed and by whom (its chat, you, going unused, or the kill switch), when its engine crashes,
  and when you pin or unpin it. Each browser tool call a chat makes is recorded with the call's
  name, the origin of its page and whether it was done, and typing names the page's origin.
  Nothing of a page's content, no typed value, no argument and no address beyond its origin is
  written (#2842).
