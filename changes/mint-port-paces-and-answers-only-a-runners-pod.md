### Security

- **The cluster operator's mint port answers a runner only from its own pod, and paces it.** A
  token is minted only for a request coming from one of the runner pod's own addresses, at most 5
  times a minute for a Claim, and one address may hold at most 4 of the port's connections at
  once. A token whose answer could not be written to the runner is revoked. A network that
  rewrites pod source addresses between pods cannot use the mint port (`docs/cluster-operator.md`).
