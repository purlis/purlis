### Fixed

- **Signing out of a forge account and back in as the same login no longer leaves every repo it
  served with no account.** Sign out now keeps the account's record, signed out, so signing in
  again as the same login gives the repos their account back. A repo whose account is signed out
  can also be given another signed-in account from Settings, even when only one is
  signed in, and Allow CI files no longer shows once the CI sign-in is given.
