### Fixed

- **A build in one branch no longer has every other branch read again.** A burst of more files
  than the branch watch holds, such as `cargo build` writing into `target/`, counted against
  every branch listened to and marked them all as moved. The cap is now counted per branch: only
  the branch over it is checked again, and a burst only where git ignores tells nobody (#4740).
