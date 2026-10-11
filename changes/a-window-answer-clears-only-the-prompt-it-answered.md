### Fixed

- **Answering one prompt in the window leaves a chat's other prompts open.** When a chat and
  one of its helpers both asked for a permission, an answer in the Inbox or on the pane cleared
  both, and the chat read as working while the other prompt still waited. Now only the prompt
  you answered goes (#1644).

### Security

- **A permission ask is read by the harness purlis started the chat as.** The window read a
  chat's permission prompt in the terms its environment named. It now reads it by the app's own
  record of the chat's harness, and leaves a prompt that names another to the chat's own pane
  (#1703).
