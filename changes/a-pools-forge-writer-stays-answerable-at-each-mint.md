### Security

- **Revoking `use` on a GitHub App key now stops minting for the Pools its holder wrote.** In the
  cluster operator, a Pool that names a key carries `runners.purlis.dev/forge-writer`, set by
  whoever writes its forge to their own name in that write, and the operator asks before each
  mint whether that person may still `use` the key, by name: a right held only through a group is
  refused. **Upgrading: a Pool written before this version mints nothing until someone who may use
  its key names themselves on it** (`kubectl annotate pool <name>
  runners.purlis.dev/forge-writer=<their name>`) (`docs/cluster-operator.md`).
