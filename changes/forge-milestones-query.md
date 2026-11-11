### Added

- **A page can show a repo's GitHub milestones live.** The new catalogue query
  `purlis.forge_milestones` answers the milestones of a repo of the page's workspace, each with
  its open and closed issue counts and due date. It reads GitHub with your own login, and a
  subscribed page is answered again about once a minute. A chat reads the same answer with the
  `forge_milestones` tool, as itself. When there is no login or no network, the answer says so in
  a sentence (#6233).
