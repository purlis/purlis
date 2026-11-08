### Security

- **The cluster holds the operator's templates and kept volumes to what their Pool says.** A new
  admission policy, `purlis-operator-writes-what-its-pool-says`, refuses the cluster operator's
  service account a pool's template whose containers run another image than the Pool's, and a
  kept claim's volume in another storage class than the Pool names, or than the one it was made
  in. It is kept when the chart is uninstalled, as the other policies are.
