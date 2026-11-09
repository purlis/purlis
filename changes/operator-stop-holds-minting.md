### Security

- **A stopped Claim or minting Pool stays stopped until someone entitled runs it again.** In the
  cluster operator, only a Claim's requester can set its `stopped` back to `false`, and only
  someone with `use` on a Pool's GitHub App key can do so for a Pool that mints, so revoking
  `use` on the key takes it away from the Pool's other writers. Turning a Pool's minting on now
  takes `use` on its key too, since it adopts the image the Pool already names. A namespace that
  was given the operator's label by hand before its policy was installed can now finish
  deleting, and `purlis-cluster-operator doctor` names any namespace outside
  `purlis-runners-*` that carries that label, with the command that removes it.
