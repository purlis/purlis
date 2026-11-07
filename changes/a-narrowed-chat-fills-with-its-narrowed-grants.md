### Security

- **A chat narrowed by a handoff or a dispatch fills and logs in with only the grants it was
  narrowed to.** Its secret fills and browser logins read the Always allows and declared logins
  of the persona it was narrowed to, not those of the persona it was started as, and its
  notice's Always allow writes in that persona's tools. The vaults it reads are still its own
  persona's.
