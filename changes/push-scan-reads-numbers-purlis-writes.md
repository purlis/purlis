### Fixed

- **The push scan no longer refuses a save over numbers purlis writes itself.** A card number,
  or a national ID's grouped form such as an Aadhaar number, is not read where it is part of a
  file's or branch's name (a session record named for the issues it closed), part of a decimal (a cost in
  dollars), or written after `#`. A card or ID number written in a sentence, or an
  ID behind its name (`Aadhaar: …`), is still caught (#5822).
- **A merge of a branch from before the scan's allowlist existed is no longer refused as an
  allowlist change.** Only a commit that changes the allowlist, or a merge that writes one
  neither side had, is. The refusal now names `.purlis-scan-allow.toml` (#5822).
- **A push from a bare repository to the remote's address leaves out what the remote already
  has**, as a push from a clone does, instead of scanning the whole history again (#5822).
