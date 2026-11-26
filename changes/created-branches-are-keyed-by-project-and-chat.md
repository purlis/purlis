### Security

- **A chat owns a branch it created only in the project it created it in.** The app's record of
  the branches a chat made through the forge route is now keyed by the project and the chat, so a
  chat of another project with the same id owns none of them and can no longer force-push them.
  Branches recorded by an earlier build carry no project and are owned by nobody: the route
  refuses updates to them, and you push those branches yourself.
