### Fixed

- **Shift+F10 straight after the keyboard reaches a row opens its menu** (#3797). When the arrow
  keys scrolled a list to a row, a Shift+F10 pressed within the same frame opened the row's menu
  and closed it again at once, so the key seemed to do nothing. A scroll closes the menu only
  when it moved the row the menu was opened on.
