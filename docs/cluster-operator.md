# The cluster operator

purlis's **cluster operator** is purlis in your cluster: a controller a team installs in its
own Kubernetes cluster, self-hosted and free (spec #2041). A team admin defines a **pool**
once: where the team's runners come from. A person asks for a runner from it with a
**claim**. The operator turns each claim into a runner, a pod running `purlis serve`, the same
session host as the person's own machine.

It is built on [kubernetes-sigs/agent-sandbox](https://github.com/kubernetes-sigs/agent-sandbox),
pinned to one tested release whose definitions purlis vendors, and it never re-implements what
that project does. Its source is [`operator/`](../operator/).

A Pool and a Claim make a running session host (#2054), the Claim says whether it is ready,
and the operator vouches for the runner's link key, so a desktop links only to the runner it
claimed (#2056, [below](#who-the-runner-is)).

## What it needs

- **Kubernetes 1.36 or later.** The operator's tests run on kind's Kubernetes 1.36.4.
- **Upstream agent-sandbox v1.0.6**, the release purlis is tested against. Its manifest,
  controller and definitions together, is vendored verbatim in
  [`operator/vendor/agent-sandbox/v1.0.6/`](../operator/vendor/agent-sandbox/v1.0.6/), with a
  kustomization beside it that names the controller's image by the digest of the release, so a
  tag moved in the registry never changes what runs. Install it with
  `kubectl apply --server-side -k`, as below.
- **A runner image**, named by digest: purlis's own or a team's that keeps the
  [runner image contract](runner-image.md).

## Its image

- **Where:** `ghcr.io/purlis/purlis-cluster-operator`, a private package of the `purlis`
  organisation on GitHub, kept private exactly as the [runner image](runner-image.md) is: the
  workflow checks through GitHub's packages API that the package is private before each push,
  and again, with the registry refusing both images to anyone who has not signed in, before
  any tag or signature. If either check cannot get a clear answer, the run stops. It stays
  private until the maintainer rules to publish it, so pulling it needs a GitHub sign-in with
  read access to the package: give the cluster an image pull secret for it.
- **When:** `.github/workflows/operator-image.yml` builds it for every change to the operator
  on `main`, once the operator's own checks and kind tests (`operator.yml`) passed on that
  commit. A pull request that changes the operator builds and checks it the same way, and
  pushes nothing.
- **What:** `linux/amd64` and `linux/arm64` under one index, each built on a GitHub runner of
  that architecture with the pinned toolchain, `cargo auditable` and the operator's `release`
  profile, no cache, into [`operator/Dockerfile`](../operator/Dockerfile). Each architecture's
  image is pushed untagged, by digest, and pulled back; nothing is tagged or signed until
  `tools/operator-image-check.mjs` has passed on exactly those bytes: Linux on that
  architecture, a numbered user who is not root, the operator as the whole default command,
  and a binary that starts there.
- **Tags:** `sha-<commit>`, and `main` for the image of the latest commit; `main` only moves
  forward. Tags can move, so name the image by its **digest**. The run's summary records it.
  Its `operator-image` artifact holds the digest and `install/`: this commit's plain
  manifests and chart, with `operator.yaml`'s image and the chart's default `image.repository`
  and `image.digest` set to that image (#2281). So an install from it names the image built
  from the same commit as the manifests, with nothing to set.
  `tools/operator-install.mjs` makes it, and the kind tests install from what it makes.
- **Signed:** keyless with [cosign](https://docs.sigstore.dev/), by the `operator-image.yml`
  workflow on `main`, through GitHub's OIDC identity, as the runner image is: no key is stored
  anywhere, and the signature's entry in Sigstore's public transparency log names the image
  and its digest (ruled 2026-10-15). Check an image before you install it:

  ```bash
  cosign verify ghcr.io/purlis/purlis-cluster-operator@sha256:<digest> \
    --certificate-identity https://github.com/purlis/purlis/.github/workflows/operator-image.yml@refs/heads/main \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

## Before you install: doctor

`doctor` says what the cluster lacks for the cluster operator, one line per finding, each with the next
step. Run it as the admin who will install, with the cluster's context current in your
kubeconfig:

```bash
cargo run --manifest-path operator/Cargo.toml --release -- doctor
```

It names the cluster it asked (your context and its API server), then checks, in order:

- **Kubernetes 1.36 or later**, from the version the API server reports.
- **Upstream agent-sandbox v1.0.6**: its four definitions, each serving `v1beta1`, and its
  controller, `agent-sandbox-system/agent-sandbox-controller`, running the release's image by
  its digest with a ready replica. The same release by its tag alone, or at another digest, is
  a finding whose next step is to apply it by digest. Not installed yet is a finding too, whose
  next step is step 1 below.
- **Your right to apply the install's cluster-scoped objects**: create and patch on
  namespaces, custom resource definitions, cluster roles and their bindings, and validating
  admission policies and their bindings, and `escalate` and `bind` on cluster roles, as
  `kubectl auth can-i` asks it. Without `escalate` and `bind`, Kubernetes refuses the install's
  cluster roles and their bindings unless you already hold every right the install's cluster
  roles grant, so doctor reports them missing.

It exits 1 when anything is missing and 0 when nothing is, so a script can gate the install on
it. It only reads: it gets the version, the definitions and the controller's deployment, and asks
the API server what you may do (`SelfSubjectAccessReview`), which the server answers and keeps
nothing of. What you may not read, or may not ask, is a finding naming the right to ask for.

```text
ok: asked context kind-team, API server https://127.0.0.1:6443/
ok: Kubernetes v1.36.4
ok: you may apply the install's cluster-scoped definitions
missing: upstream agent-sandbox v1.0.6: not installed. Next: apply it, step 1 of the install: kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6.
1 missing: install the cluster operator once doctor finds nothing missing
```

## Install

A Helm chart, [`operator/charts/purlis-cluster-operator/`](../operator/charts/purlis-cluster-operator/),
or the plain manifests in [`operator/deploy/`](../operator/deploy/), which are rendered from the
chart and make exactly the same objects: `operator/tests/the_chart.rs` holds the two equal, and
the kind tests install the chart and check that the plain manifests would change nothing. The
chart is not published to any chart repository; install it from a checkout of this repository.

Either way the operator's image is named by its **digest**: the chart refuses an install
without one, and the plain `operator.yaml` names a placeholder digest no registry holds until
you set the real one.

### With Helm

Helm 4, the version the tests use. Four steps, in order:

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller, named by
#    the release's digest.
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6

# 2. The release's namespace. Only pods that meet the restricted Pod Security Standard are
#    admitted in it; the operator's pod does.
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted

# 3. The key the operator signs runners' link keys with, made once, here, and kept as a Secret
#    the operator's pod mounts. Keep it: a new key makes every desktop ask again.
#    Piped, so the key is never written to a file.
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin

# 4. purlis's definitions, the policy on Claims, and the operator.
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system \
  --set image.digest=sha256:<digest> \
  --set 'imagePullSecrets[0].name=<a secret that pulls ghcr.io/purlis>'
```

**The definitions are a step of their own.** Helm installs the chart's `crds/` (Pool and
Claim) before anything else in it, and never upgrades or deletes them. A cluster admin who
reviews and applies them apart from the controllers applies `operator/deploy/crds.yaml` with
`kubectl apply --server-side` and installs the chart with `--skip-crds`. After an upgrade of
the chart that changes them, apply `operator/deploy/crds.yaml` the same way, before the chart:
Helm never upgrades `crds/`. An upgrade to a chart whose policy asks for a Claim's `requester`
without the definitions that hold it refuses every new Claim until they are applied.

`helm uninstall` removes the operator and leaves the definitions, the Claims and Pools in
them, and the policy on Claims, which Helm keeps (`helm.sh/resource-policy: keep`).

The chart's values, all in its
[`values.yaml`](../operator/charts/purlis-cluster-operator/values.yaml), and
`values.schema.json` refuses any other:

| Value | What it sets | Default |
|---|---|---|
| `image.repository` | where the image is pulled from, without a tag or digest | `ghcr.io/purlis/purlis-cluster-operator` |
| `image.digest` | the image's digest, `sha256:` and 64 hex digits; required | none |
| `imagePullSecrets` | secrets in the release's namespace that pull the image | none |
| `resources` | the operator's CPU and memory | requests 10m CPU and 32Mi, limit 256Mi |

No value changes what the operator may do or how its pod is confined.

### With the plain manifests

The same steps with `kubectl`. The definitions and their policy are a step of their own, so a
cluster admin can review and apply them apart from the controllers.

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller, named by
#    the release's digest.
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6

# 2. The policy that admits a Claim only for someone who may use its Pool, then purlis's
#    definitions, Pool and Claim. The policy goes first, so no Claim is admitted before it
#    holds.
kubectl apply --server-side -f operator/deploy/policy.yaml
kubectl apply --server-side -f operator/deploy/crds.yaml

# 3. The `purlis-system` namespace and the key the operator signs runners' link keys with,
#    kept as a Secret its pod mounts. Keep it: a new key makes every desktop ask again.
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
#    Piped, so the key is never written to a file.
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin

# 4. The operator, with its image named by digest: the `install/operator.yaml` of a run's
#    `operator-image` artifact, which already names it, or this one with the image set.
kubectl apply --server-side -f operator/deploy/operator.yaml
kubectl -n purlis-system set image deployment/purlis-cluster-operator \
  operator=ghcr.io/purlis/purlis-cluster-operator@sha256:<digest>
```

A build of your own, from [`operator/Dockerfile`](../operator/Dockerfile), is installed the
same way, with its own repository and digest.

## A pool and a claim

```yaml
apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team
spec:
  image: ghcr.io/purlis/purlis-runner@sha256:<digest>
---
apiVersion: runners.purlis.dev/v1alpha1
kind: Claim
metadata:
  name: ada-1
  namespace: ada
spec:
  pool: team
  requester: ada@example.com
```

- **A Pool** is cluster-wide, defined once for the team. Its `image` is a runner image named
  by digest (`<repository>@sha256:<digest>`); the cluster refuses an image named by tag, since
  a tag can move.
- **A Claim** lives in a namespace and names its `pool`, which never changes: another pool is
  another claim. Its `Ready` condition says whether its runner is running, and if not, why:
  `PoolNotFound` when there is no such pool, `PoolDeleted` when it was deleted after the
  runner was asked for (below), `UpstreamObjectInTheWay` when an upstream object the operator
  did not make has the name it would use (below), `HeldByClaimsMadeBeforePolicy` when the
  pool's objects there belong only to Claims made before the policy on Claims was in force
  (below), `Starting` until upstream reports, and upstream's own reason after that.
  `kubectl get claims` shows it.
- **A Claim names who makes it**, `requester`, exactly as the cluster signs them in
  (`kubectl auth whoami`): the policy refuses a Claim whose `requester` is anyone else, in words
  that name the right one, and it never changes after. The operator vouches for it with the
  runner's link key. purlis fills it in when it claims a runner.
- **Deleting a Claim** deletes its runner.
- **Deleting a Pool stops nothing that runs.** A Claim whose runner was asked for keeps it,
  running as the pool last defined it, until the Claim is deleted: its `Ready` is still what
  upstream says of the runner, `True` while it runs, with the reason `PoolDeleted`. No new
  Claim is served from a deleted pool: one made after says `PoolNotFound`. A pool made again
  under the same name serves new Claims, and its definition is applied again to the template
  the kept runners came from.

`Pool` and `Claim` are `v1alpha1`: no stability promise before upstream agent-sandbox leaves
beta.

**Who may use a Pool is the cluster's own access control.** A Claim is admitted only when the
person making it may `use` the Pool it names: a ValidatingAdmissionPolicy
([`operator/deploy/policy.yaml`](../operator/deploy/policy.yaml)) asks the cluster's
authorizer, as that person, when the Claim is created. Anyone else's Claim is refused with
the pool's name. A cluster admin grants `use` with RBAC, on one pool or on all of them:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: use-pool-team
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [pools]
    resourceNames: [team]
    verbs: [use]
```

Who may create a Claim, and in which namespace, is RBAC on `claims` as usual.

**A runner from a pool is had only through a Claim.** The same policy refuses an upstream
`SandboxClaim` on one of a pool's warm pools, and an upstream `SandboxWarmPool` of one of a
pool's templates (both named `purlis-pool-<pool>`, below), when it is made there, moved there or
moved off one by anyone but the operator's service account: such a runner would skip the checks
above. Upstream's own objects that are not a pool's are left to upstream. This holds as far as
Kubernetes' usual boundary goes: whoever may act as the operator's service account (impersonate
it, mint its tokens, or run a pod as it in the operator's namespace), or change the policy and its
binding, passes it. Rights in the operator's namespace are the operator's rights.

## Who the runner is

A desktop links to a runner over ADR 0078's encrypted link, and links only to the runner it
claimed. Two witnesses say which key that runner holds, and both must agree:

1. **The runner makes its link key as its pod starts.** The pod's first step, `link-key`, runs
   the pool's image (`purlis bridge --make-key`) before anything else in the pod: it makes the
   key, hands its private half to the runner in a folder in memory the two containers alone
   share (`/run/purlis-link/`), and writes its public half as its termination message, which
   the cluster shows in the pod's status.
2. **The operator signs what it saw.** It follows the claim to its pod by uid: its upstream
   claim, the sandbox that claim controls, and the pod that sandbox controls. It holds the pod
   to the template it made for the pool in full, every field but the few the cluster itself
   sets or defaults, and refuses a pool's template that someone else changed. Then it signs a statement of the claim, the pod, the key and the requester with
   its own Ed25519 key. The statement and its signature are the Claim's `status.linkKey`, and
   its `KeyPublished` condition says `Vouched`, or why not yet. Every Pool's
   `status.signingKey` names the operator's key, which a desktop pins when it adds the pool.
3. **The desktop pairs over its own `kubectl exec`** into the pod the statement names, as a
   Docker pool's desktop pairs over `docker exec`, and the runner answers with its device id
   and key. A statement signed by another key, about another claim or requester, or a runner
   answering another key than the one vouched for, is refused, and so is a statement that names
   another pod or key before any later link. The pairing takes the private half out of the
   shared folder, so it is left only in the session host's own. The operator looks at a
   vouched claim again every minute, and takes its statement away once it no longer holds.

**Vouching is only as strong as the namespace is the requester's own.** Anyone who may exec into
pods, make pods, or change the pool's objects in a claim's namespace can reach its runner, read
its key, or pair with it first, whatever the statement says. Until the operator makes a
namespace per person (#2055), give each person a namespace of their own and no one else rights
in it. Read access to Secrets in the operator's namespace is the power to vouch for any runner.

A runner is paired once: a pod whose container restarts has lost its pairing, and the claim is
made again.

**The operator acts only on Claims admitted under its policy, however it was installed.** It
reads the policy's binding before it acts on any Claim. While no binding puts the policy in
force, it acts on no Claim, and each says `PolicyNotInForce`. The policy is taken to be in force
30 seconds after its binding was made, the time every API server takes to enforce it. A Claim
made before then is never acted on: it says `MadeBeforePolicy`; delete it and make it again.
So wait 30 seconds after installing before making the first Claims.

Making the binding again, by reinstalling the policy or recreating the binding by hand, refuses
every Claim that already exists: each says `MadeBeforePolicy` and has to be deleted and made
again. A runner such a Claim already has keeps running, with the template and warm pool it was
made from, until the Claim is deleted. Only a Claim the operator acts on makes those objects
count as the operator's, so a Claim made since, on that pool in that namespace, says
`HeldByClaimsMadeBeforePolicy`, naming the older Claims, and waits until they are deleted.

## What it makes

For a Claim in namespace `ns` on Pool `p`, the operator makes, and then applies server-side:

| Upstream object | Name | What it holds |
|---|---|---|
| `SandboxTemplate` | `purlis-pool-p` in `ns` | the runner's pod: its first step, `link-key`, making the runner's link key (above), then one container running the pool's image with its own entrypoint (`purlis serve`), both as a user who is not root, every capability dropped, no privilege escalation, the runtime's default seccomp profile, no Kubernetes credential mounted and no service addresses in their environment |
| `SandboxWarmPool` | `purlis-pool-p` in `ns` | no runner kept warm: every claim starts one |
| `SandboxClaim` | the Claim's own name in `ns` | the claim on that warm pool |

The upstream claim belongs to the Claim, and the template and the warm pool in a namespace to
the pool's Claims there, each Claim adding itself, so the cluster's garbage collector removes the
upstream claim with its Claim and the template and warm pool with the last of their Claims, and
never while a Claim in the namespace still has a runner from them. A Claim made as the last one
goes is served all the same: if the garbage collector removes the template or warm pool it has
just joined, the operator makes them again at once. Upstream's controller makes the pod
from the template and writes the upstream claim's status, which the operator reads.

**It changes only objects it made.** Before it writes any of the three, it reads what the
cluster holds under that name. An upstream claim it made names its Claim as its controller, by
uid, and a template or warm pool it made is owned by one of the pool's Claims in that
namespace, by name and uid; any other object under that name, one made by hand or by another tool, is left
exactly as it is. The Claim then says `UpstreamObjectInTheWay`, naming the object's kind and
name, and nothing is made for it. Remove or rename that object and the Claim goes on at once: the
operator watches all three kinds.

## What it may do

Its ClusterRole, in [`operator/deploy/operator.yaml`](../operator/deploy/operator.yaml), is all
it holds:

| API group | Resources | Verbs |
|---|---|---|
| `runners.purlis.dev` | `pools` | get, list, watch |
| `runners.purlis.dev` | `claims` | list, watch |
| `runners.purlis.dev` | `claims/status` | patch |
| `runners.purlis.dev` | `pools/status` | patch |
| core | `pods` | get |
| `agents.x-k8s.io` | `sandboxes` | get |
| `admissionregistration.k8s.io` | `validatingadmissionpolicybindings`, only `purlis-claims-use-their-pool` | get, list, watch |
| `extensions.agents.x-k8s.io` | `sandboxclaims`, `sandboxtemplates`, `sandboxwarmpools` | create, get, list, patch, watch |

It holds no right to `exec`, `attach` or port-forward to a pod and none to read a Secret, and it
reads a sandbox and a pod only by name: a Claim's, which upstream made, to follow the claim to
its runner and read the key it vouches for. It never lists, makes, changes or deletes a pod. Its own signing key is the
Secret you made at install, which the kubelet mounts in its pod. Its Kubernetes client is built
without the websocket support `exec` would need.
`operator/tests/the_install_manifests.rs` holds these rules, and the kind tests ask the cluster
itself. It runs as a user who is not root, with a read-only root file system and every
capability dropped.

Upstream's controller holds its own ClusterRoles, in the vendored manifest: it makes and
removes the runners' pods, services, volume claims and network policies. It holds no Secret
right and no `exec` right either.

**Cluster admins and node owners can read everything in the cluster**, every runner's files
and memory included. Give cluster-admin and node access accordingly.

## Tests

- `cargo test` in `operator/`, with `helm` 4 on PATH: what a Claim asks of upstream, that
  every field the operator writes is one the pinned upstream declares, what doctor finds on
  clusters the tests describe (and that it only reads), that the operator acts only on Claims
  admitted under its policy, the install manifests, and the chart, rendered and linted by Helm
  itself: the plain manifests are what it renders, its image is named only by digest, and no
  value changes what the operator may do. The definitions are generated from the Rust types and
  the plain manifests from the chart; regenerate them, in this order, with
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests` and
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart`.
- [`operator/e2e/kind.sh`](../operator/e2e/kind.sh) `<runner image> <operator image>`: a kind
  cluster of Kubernetes 1.36 with a registry beside it, so images are named by digest; doctor,
  which must name upstream agent-sandbox as missing; the recorded controller digest checked
  against the registry; the pinned upstream, its controller by that digest, after which doctor
  must find nothing missing; purlis's definitions, the policy and the operator installed by the
  chart in a namespace that enforces the restricted Pod Security Standard; `kubectl diff`
  showing the plain manifests would change nothing the chart made; then
  `operator/tests/on_a_kind_cluster.rs`, which checks only what the cluster shows; then the
  desktop's side, `crates/purlis-core/tests/a_chat_runs_on_a_cluster_runner_through_kubectl_exec.rs`:
  a Claim, the operator's statement checked against the Pool's key, a pairing over
  `kubectl exec` that must agree with it (and one reaching another runner, refused), then a
  chat on the runner through the link; then `helm uninstall`, which leaves the definitions and
  the policy. It removes everything it started. `.github/workflows/operator.yml` runs it, with
  a runner image built from the same commit, on every pull request that changes `operator/`,
  `runner-image/` or the desktop's side of a cluster runner.
- The statement both sides read is one worked example in
  [`fixtures/runner-link-key/`](../fixtures/runner-link-key/), which the operator's tests sign
  and the desktop's tests verify, byte for byte.

## Moving the upstream pin

Upstream releases weekly. To move to another release: vendor its
`sandbox-with-extensions.yaml` under `operator/vendor/agent-sandbox/<version>/` in place of the
old one, with the old one's `kustomization.yaml` moved beside it; set `VERSION` in
`operator/src/upstream.rs`, and the path and tag in `operator/e2e/kind.sh`; take the
controller image's digest for the new tag from Kubernetes' image promoter
([`registry.k8s.io/images/k8s-staging-agent-sandbox/images.yaml`](https://github.com/kubernetes/k8s.io/blob/main/registry.k8s.io/images/k8s-staging-agent-sandbox/images.yaml))
and set it in the kustomization (`newTag` and `digest`) and as `CONTROLLER_DIGEST` in
`upstream.rs`; then run the tests. `upstream_is_installed_by_digest` fails while the
kustomization, the vendored manifest and `upstream.rs` disagree on the release or the digest,
and the kind run fails when the registry holds another digest under the tag.
`only_what_the_pinned_upstream_defines_is_written` fails when a field the operator writes is
gone or renamed, before a cluster does.
