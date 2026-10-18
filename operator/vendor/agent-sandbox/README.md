# Upstream agent-sandbox, vendored

`v1.0.6/sandbox-with-extensions.yaml` is the release asset of the same name of
[kubernetes-sigs/agent-sandbox v1.0.6](https://github.com/kubernetes-sigs/agent-sandbox/releases/tag/v1.0.6),
byte for byte: its definitions (`Sandbox`, `SandboxTemplate`, `SandboxWarmPool`,
`SandboxClaim`), its controller and the controller's permissions.

- SHA-256: `e6fa909d82392b201ef80003dd91390450266ba356caa9be04a1c89c3caa65dc`
- Licence: Apache-2.0, copyright The Kubernetes Authors.

`v1.0.6/kustomization.yaml` is purlis's, not upstream's: it applies that manifest unchanged but
for its controller's image, which it names by the digest of the release, as Kubernetes' image
promoter published it. Install with `kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6`.

purlis's cluster operator is tested against exactly this release (`VERSION` in
`operator/src/upstream.rs`); docs/cluster-operator.md says how to move it.
