### Changed

- **Starting a chat on a pool sends only the profile settings that changed** (#4088). A profile's
  skills and settings used to cross whole at every start. Now the runner says what it holds, and an
  unchanged profile sends none of them. The runner still checks the
  whole result, and a file changed or added there is put right at the next start. A runner of an
  older version is still sent everything.
