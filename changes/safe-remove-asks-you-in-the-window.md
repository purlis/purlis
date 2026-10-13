### Fixed

- **Safe remove no longer ends in a refusal.** When a chat runs `purlis workspace remove`
  under the sandbox, purlis now asks you on that chat's tab and in the Inbox: **Remove** or
  **Keep**. Remove does what Delete workspace in the window does, with the same guard, and is
  never forced; if the workspace holds work nothing else has, the Notice says so and opens
  Delete workspace. Nothing is removed unless you press Remove. The chat is told what you
  decided on its next turn. Before, the command was refused and you had to remove the
  workspace yourself. Removing a persona or a branch folder from a sandboxed chat is still
  refused.
