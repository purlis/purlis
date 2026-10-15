### Added

- **The picker a chat starts from shows who pays.** Each profile's row says its model (the
  harness's default for now) and what a chat on it is billed to: your plan, your key, your cloud
  account, or nobody for a local model, with the profile's label, or whatever the harness is
  signed in with where purlis cannot see it. A chat's run records its profile's sign-in kind as
  its model source (#2036).
- **A cloud profile signs its chats in from your vault.** A profile of this machine that names
  its Bedrock or Azure credentials by vault reference has them read when a chat starts, from a
  vault the chat's persona may use, and handed to that chat's harness alone. A key profile
  waits for the key proxy, and its row says so (#2036).
