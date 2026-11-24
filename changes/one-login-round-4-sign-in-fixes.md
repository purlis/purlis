### Fixed

- Asking GitHub for the CI-file permission, then cancelling, letting the code expire or quitting
  purlis, no longer leaves your everyday sign-in in use by chats while purlis cannot tell what it now
  allows. The account is held back from chats as soon as the request starts. A retry that reads the
  sign-in's permissions back, or a new sign-in once GitHub's code has lapsed, gives it back.
- A sign-in made with a pasted fine-grained token now says that purlis cannot check what it may do
  and does not give it to chats, instead of suggesting you sign in again.

### Security

- A GraphQL request that names several of a chat's repos is sent only when one account serves all of
  them, and with that account's token; a request outside any repo needs every repo of the chat on
  that host to be served by the same account.
- On GitHub, a sign-in whose token or grant names the CI-file permission is recorded as holding it.
