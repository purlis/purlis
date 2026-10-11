### Changed

- **`purlis workspace create` works from a sandboxed chat.** In a chat purlis started,
  `purlis workspace create <name> [--vision "…"]` hands the create to purlis, which makes the
  new workspace as a terminal's `create` does and prints what it always printed. It makes only a
  workspace that is not there yet. `--live`, `--use` and repos to clone still run in the chat
  (#1408).
- **`--workspace <its own workspace>` in a chat is the same as no `--workspace`.** In a chat purlis
  started, `purlis workspace remember`, `note`, `todo`, `todo done|forget`, `vision`, `forget`,
  `edit`, `archive` and `unarchive` hand their write to purlis when `-w` names the chat's own
  workspace, as they did with no `-w`. A sandboxed chat naming another workspace is refused
  with a sentence that names the way round (#1408).
