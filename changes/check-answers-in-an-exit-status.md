### Changed

- **`purlis profile sign-in <name> --check` answers in its exit status.** It exits 0 if signed in, 3 if
  not, 4 if its harness cannot tell, 5 while its harness is being fetched, 6 if there is no such profile,
  and 1 if it could not ask. A harness that cannot tell used to exit 0, so a script that reads 0 as "go
  ahead" should accept 4 and 5 too (#4113).
