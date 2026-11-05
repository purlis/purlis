# Amended 2026-11-05: the foot's presses are rows of the chat's list

Ticket #5204, from building spec #5191. The spec says each part of the foot presses a row that
already exists in ⋯ and the palette. Only `ctx`'s did (Activity): the harness's card was in the
palette but not in ⋯, and a chat's workspace, the chat it came from and its skills had no row. The
rule the spec gives for it is that nothing lives on the foot only, so the rows the foot needs were
added to the chat's list and the palette rather than given a press of their own:
`chat.harness`, `chat.workspace`, `chat.asker`, `chat.skills` and `chat.studio`.

- **A part whose row cannot run is its words.** A chat's workspace is usually the focused one, whose
  row greys with its reason; the foot then says the workspace in words, since a press would go
  nowhere.
- **The foot says the skills warning, and no count while nothing is wrong.** A quiet `N skills` on
  every chat's foot would be noise on a line that is meant to be read at a glance, and the prototype
  draws nothing there. The way to a chat's skills is its Skills row in ⋯ and the palette.
- **What the Skill studio started a chat for is on the foot, beside `↳ from`** (#5201, which the spec
  does not name): it says where the chat came from. So is its work item, for the same reason.
- **A shell and a one-job program keep their heads**, sandbox, `↳ from` and work item among them. A
  skills warning and a studio chip cannot apply to either: neither has a set of skills, and the
  studio starts neither.
