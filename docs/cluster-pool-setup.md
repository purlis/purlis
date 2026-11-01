# Set up a pool on your cluster

This guide takes a Kubernetes cluster with no purlis on it to a pool you can run chats on. It
takes about half an hour, most of it waiting for images and a first build. Each step gives the
exact command and what you should see. For the detail behind a step, see
[The cluster operator](cluster-operator.md).

**A pool** is where runners come from. **A runner** is a pod in your cluster that runs chats for
one person. **The cluster operator** is the small controller that makes runners. You install it
once per cluster.

CI runs every command here on a kind cluster on each change to this guide or the operator,
unless the line above it says it hasn't been run.

## What you need

- A cluster running Kubernetes 1.36 or later, and a kubeconfig context for it with admin rights.
  It has to be the current context while you work through this guide.
- `kubectl`, Helm 4, `openssl`, `git`, Docker and a [Rust toolchain](https://rustup.rs) on your
  machine. Rust is only for the doctor in step 1.
- A registry your cluster's nodes can pull from.
- purlis on your machine, for the last step.
- **Read access to purlis's images.** They are private for now, so you need a GitHub sign-in that
  can read `ghcr.io/purlis`. Without that access you can't finish this guide yet. Publishing the
  images (#4498) removes this need, along with step 2.

Get the files the steps use, and run everything after this from that folder:

_Not yet run in CI: CI starts from a checkout already._

```bash
git clone https://github.com/purlis/purlis.git
cd purlis
```

## 1. Ask the doctor what's missing

The doctor only reads. It says what the cluster lacks and what to do about each thing.

<!-- kind: doctor -->

```bash
cargo run --manifest-path operator/Cargo.toml --locked --quiet -- doctor
```

The first run builds it, which takes a few minutes. On a cluster with nothing on it you should
see this, with your own context and address:

```text
ok: asked context kind-team, API server https://127.0.0.1:6443/
ok: Kubernetes v1.36.4
ok: you may apply the install's cluster-scoped definitions
missing: upstream agent-sandbox v1.0.6: not installed. Next: apply it, step 1 of the install: kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6.
1 missing: install the cluster operator once doctor finds nothing missing
```

The agent-sandbox line is expected here, and step 3 fixes it. Fix anything else it reports first.

## 2. Copy the images to your registry

A runner's pod has no way to name a pull secret, so your nodes have to be able to pull its image.
Copying both images to your own registry covers that. Copying keeps their digests.

_Not yet run in CI: it needs a sign-in that can read the private images._

```bash
docker login ghcr.io
docker buildx imagetools create --tag <your registry>/purlis-cluster-operator:main ghcr.io/purlis/purlis-cluster-operator:main
docker buildx imagetools create --tag <your registry>/purlis-runner:main ghcr.io/purlis/purlis-runner:main
docker buildx imagetools inspect <your registry>/purlis-cluster-operator:main --format '{{json .Manifest.Digest}}'
docker buildx imagetools inspect <your registry>/purlis-runner:main --format '{{json .Manifest.Digest}}'
```

The last two commands each print a digest that starts with `sha256:`. Keep them: the first is the
operator's digest for step 5, and the second is the runner's for step 6.

## 3. Install agent-sandbox

The operator is built on this project. This installs the exact release purlis is tested with.

<!-- kind: upstream -->

```bash
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6
kubectl -n agent-sandbox-system rollout status deployment/agent-sandbox-controller --timeout=180s
```

You should see `successfully rolled out`. Run the doctor from step 1 again. It should now print
only `ok:` lines.

## 4. Make the signing key

The operator uses this key to vouch for each runner, so a desktop only links to the runner it
claimed. Make it once and keep it. Running this a second time fails because the Secret exists,
and that's what you want: replacing the key makes every desktop check the pool again. To change
it later, see [Rolling the signing key](cluster-operator.md#rolling-the-signing-key).

<!-- kind: key -->

```bash
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin
```

You should see `namespace/purlis-system created`, then `labeled`, then
`secret/purlis-cluster-operator-signing-key created`.

## 5. Install the operator

Put in your registry and the operator's digest from step 2.

<!-- kind: install -->

```bash
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system \
  --set image.repository=<your registry>/purlis-cluster-operator \
  --set image.digest=<operator digest>
```

Then wait for it:

<!-- kind: installed -->

```bash
kubectl -n purlis-system rollout status deployment/purlis-cluster-operator --timeout=120s
```

You should see `successfully rolled out`. If it stays waiting, your nodes probably can't pull the
image: `kubectl -n purlis-system describe pods` says so under Events.

## 6. Make your first pool

Wait a minute after step 5: the operator only acts on Claims made more than 30 seconds after its
policy is in place. Put in your registry and the runner's digest from step 2. Change `team` if you
want another name.

<!-- kind: pool -->

```bash
kubectl apply -f - <<'EOF'
apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team
spec:
  image: <your registry>/purlis-runner@<runner digest>
EOF
```

You should see `pool.runners.purlis.dev/team created`. Now print the pool's key, which you'll check
in the next step:

<!-- kind: pool-key -->

```bash
kubectl get pools.runners.purlis.dev team --output jsonpath='{.status.signingKey}'
```

It can print nothing for a few seconds. Run it again until it prints the key.

## 7. Connect from the window

_Not yet run in CI: these are presses in the window. CI runs the same claim and link from the core,
in `a_chat_runs_on_a_cluster_runner_through_kubectl_exec`._

1. Open Settings › This machine › Pools and press **Add a pool…**, or choose **Add a pool…** in the
   palette.
2. Set **Kind** to Cluster.
3. Give it a **Name**, leave **Kubeconfig** empty unless yours is somewhere unusual, choose the
   **Context** and type `team` as the **Pool**.
4. Press **Check its key**. The key shown must be the one step 6 printed. If it isn't, stop: you
   may be talking to another cluster.
5. Press **Add pool**. Its runner is claimed and paired, which can take a few minutes while the
   node pulls the image.

The pool now shows in Settings › This machine › Pools, ready for chats to run on.

## Let a teammate use the pool

Each person needs the right to use the pool and to make Claims in their kubeconfig's namespace.
Put in the name the cluster knows them by, which `kubectl auth whoami` prints for them. Their
namespace is `default` unless their context names another.

<!-- kind: teammate -->

```bash
kubectl apply -f - <<'EOF'
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: use-pool-team
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [pools]
    resourceNames: [team]
    verbs: [get, use]
  - apiGroups: [runners.purlis.dev]
    resources: [claims]
    verbs: [create, get, patch, delete]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: use-pool-team
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: use-pool-team
subjects:
  - apiGroup: rbac.authorization.k8s.io
    kind: User
    name: <their name>
EOF
kubectl auth can-i use pools.runners.purlis.dev/team --as <their name>
```

You should see `yes` on the last line. That binding lets them make Claims in any namespace. To hold
them to one, bind the Claim rights there with a RoleBinding instead. Give them the pool's key too,
so they can check it in step 7. They only need step 7, and the operator gives them a namespace of
their own for their runners.

## If something goes wrong

- **The window says the cluster has no such pool or resource:** the operator isn't installed on
  that context's cluster, or the Pool has another name. Check with `kubectl get pools`.
- **The add waits and then gives up:** run `kubectl get claims -A`. Its `Ready` condition says why
  the runner isn't up, for example `Pulling` with the kubelet's words when the node can't pull.
- **Anything else:** run the doctor from step 1 again.
