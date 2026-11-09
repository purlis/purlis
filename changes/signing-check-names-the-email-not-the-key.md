### Fixed

- **Set up signing's Check no longer tells you to add a key that's already on your account.**
  GitHub matches a signed commit to an account by the commit's email, so when the email isn't one
  of the account's verified ones it says it doesn't know the key. Check now sees that the account
  lists this device's key and says the email is why, then offers the account's verified emails to
  sign chat commits as. Check also always names the email it committed as.
