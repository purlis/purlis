### Changed

- **Making a workspace takes you there, and its notice hangs from its chip** (#3648). Creating
  a workspace now opens it and pins it, even with a chat in front. Its "Workspace X is ready"
  notice no longer pushes the window down: it hangs from the new workspace's chip in the title
  bar, as a card with a point, offering Add a vision (while it has none) and Details, and no
  longer Open. A click elsewhere hides the card and leaves a ● dot on the chip, which brings it
  back; × or Esc dismisses it. F6 reaches the card. It is still listed in the Notices centre
  under Results, and dismissing it in either place dismisses it in both.
