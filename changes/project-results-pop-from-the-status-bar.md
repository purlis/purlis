### Changed

- **A project's or the app's result pops from the status bar** (#3653). A refused pin and the
  other results of project and app actions no longer push the window down from the top of the
  content: they pop from the status bar's Notices item, as the same card a workspace's notice is
  on its chip, pointing down at Notices. A click elsewhere hides the card and leaves a ● dot on
  Notices, which brings it back; × or Esc dismisses it, and F6 reaches it. Each is still listed
  in the Notices centre under Results, and dismissing it in either place dismisses it in both.
  Themes reach the new dot as `status-item.dot`.
