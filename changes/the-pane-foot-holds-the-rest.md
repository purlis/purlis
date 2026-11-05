### Changed

- **The rest of a chat's facts moved to its pane foot.** Where the chat came from, what the Skill
  studio started it for, its work item, its skills warning and its sandbox are on the quiet line
  under the terminal now, and each part you can press runs a row of the chat's ⋯. Those rows are new
  in ⋯ and the palette: the harness's card, the chat's workspace, the chat it came from and its
  skills. The count of a chat's skills is no longer on its pane unless something needs you (#5204).
- **A pane's foot says what the pane is running.** A pane showing a task says the task's harness,
  gauge and skills, and `↳ from` names the chat that asked. An ended chat keeps its foot, its gauge
  frozen at its last values, greyed and marked `ended` (#5205).
- **A narrow pane drops parts from its head and its foot in a fixed order.** The foot drops `cache`,
  then shrinks the skills warning to its ⚠, drops `↳ from`, then the workspace. The harness's name,
  ⚠ and `ctx` always stay (#5206).
