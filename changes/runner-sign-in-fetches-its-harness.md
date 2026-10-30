### Fixed

- **Signing a profile in on a fresh runner works** (#4007). Settings › Pools › Sign in no longer
  fails with "purlis could not find a program called claude" on a runner where no chat has
  started yet. The runner first fetches the harness version the profile pins, from its vendor's
  release and checked against the SHA-256 the vendor publishes, the same as a chat's first start
  does, and the sign-in shell says it is fetching. A fetch that cannot reach the vendor or does
  not match its checksum says why and signs nothing in. The profile's row stops saying its
  harness cannot say whether it is signed in once that version is on the runner: asking about
  the profile starts the fetch in the background, and Ask again shows the answer.
