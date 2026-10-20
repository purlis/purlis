### Fixed

- **A chat's browser keeps the persona its sandbox was started for.** After a handoff, or for a
  chat on no persona, the sites its browser reaches and the Asks about them follow the persona
  whose grants the chat started with, until it starts again. Its browser logins still use the
  vaults of the persona it was started as (#2691).
- **The Notice of the project's browser sites says nothing in a project without the sandbox**,
  where those sites reach no browser. A site you were shown is no longer told as taken out when
  the project turns the sandbox off (#2691).
