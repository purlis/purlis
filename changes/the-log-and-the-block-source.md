### Changed

- **A refusal a family lets through now goes in the event log.** A family or rule you set to `log`
  writes one quiet would-have-blocked event under the chat, naming the rule, the family, the chat
  and the host, and never a command, a path or a URL. The separate enforcement log
  (`app/enforcement/<chat>.jsonl`) and the `$PURLIS_POLICY` variable are gone: a chat's hooks ask
  purlis how each family runs. A chat reads only its own rows of the event log. Every block names
  its source: a rule, the sandbox (and the rule it was compiled from) or the harness's own check
  (M75-13, #7022).

### Security

- **With the sandbox off, the event log can no longer be rewritten unseen.** purlis chains each
  line to the one before it and seals the log's head in your keyring at each write. At each chat's
  start and in doctor it checks the chain and the sealed head, and names the first line that no
  longer matches. A log that was edited, cut or recomputed is reported (M75-13, #7022).
