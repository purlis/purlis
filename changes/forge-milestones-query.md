### Added

- **A page can show a repo's GitHub milestones live.** The new catalogue query
  `purlis.forge_milestones` answers the milestones of a repo of the page's workspace, each with
  its open and closed issue counts and due date. It reads only the repo the project records for
  that clone, and refuses a clone whose origin was changed. It reads GitHub with your own login,
  within purlis's hourly request budget on either a purlis sign-in or `gh`, and a subscribed page
  is answered again at most once a minute. It reads the first 300 milestones and says when there
  are more. A chat reads the same answer with the `forge_milestones` tool, as itself. When there
  is no login or no network, the answer says so in a sentence (#6233).
