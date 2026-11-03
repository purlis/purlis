### Changed

- **A busy chat behind the edge of the chat strip no longer redraws the project.** The show-more
  menu lists the tabs the strip has no room for, most recently moved first. That order used to
  be worked out for the whole project view, so every move of a hidden chat redrew all of it. It
  is now worked out by the menu alone, so with many busy chats open, background activity costs
  the window only that menu (#4726).
