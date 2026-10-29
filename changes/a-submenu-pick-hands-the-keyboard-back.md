### Fixed

- **The keyboard goes back where it was after a submenu pick or a row's menu, inside a dialog
  too.** A row picked in a submenu now hands the keyboard back to the menu's trigger, and a menu
  opened on a list's row hands it back to that row, the moment the menu goes. Before, in a dialog
  opened less than half a second earlier, the dialog could take the keyboard instead (#3642).
