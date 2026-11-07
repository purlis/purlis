### Changed

- **A browser login a chat suggests says when its vault has nothing under a key it names.** The
  notice reads "Vault qa has nothing under password yet", so a misspelt key is seen before you
  allow it, and an Always line doesn't write it unseen. Allowed anyway, purlis asks you for the
  value when it signs in. Vaults kept by 1Password aren't listed for this, since listing one can
  ask you to unlock it.
