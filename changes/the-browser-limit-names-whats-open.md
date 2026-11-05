### Changed

- **A chat at the browser limit is told what its browsers hold.** The refusal lists that chat's
  browsers: empty, on an error page, on which site, holding which login, or taken over by you.
  Other chats' browsers are counted and not named, and no value is shown (#5296).
- **A login kept for reuse leaves no empty browser behind.** When it opens from what is kept in a
  browser of its own, purlis closes the browser the chat just opened and never used, and says so
  in the answer and in your view of the browser. At the project's limit that makes the room for it.
  A browser with anything done in it, or one you took over or pinned, stays open (#5296).
