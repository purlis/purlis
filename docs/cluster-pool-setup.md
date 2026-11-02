# Set up a pool on your cluster

This guide takes a Kubernetes cluster with no purlis on it to a pool you can run chats on. It
takes about half an hour, most of it waiting for images and a first build. Each step gives the
exact command and what you should see. For the detail behind a step, see
[The cluster operator](cluster-operator.md).

**A pool** is where runners come from. **A runner** is a pod in your cluster that runs chats for
one person. **The cluster operator** is the small controller that makes runners. You install it
once per cluster.

CI runs each command block here on a kind cluster, unless a line above it says otherwise. The
commands under "If something goes wrong" aren't run in CI.

<!-- This file's named bash blocks run in CI through operator/e2e/kind.sh. Review them as code. -->

## What you need

- A cluster running Kubernetes 1.36 or later, with a default storage class and nodes on Linux 6.12
  or later with Landlock. A runner's volume uses the default class, and sandboxed chats need that
  kernel. You also need a kubeconfig context for the cluster with admin rights, and it has to be
  the current context while you work through this guide.
- `kubectl`, Helm 4, OpenSSL 3, `git`, `gh`, `curl`, `jq`, cosign 3, Docker and a
  [Rust toolchain](https://rustup.rs) on your machine. Rust is only for the doctor in step 1. `gh`
  has to be signed in, to any GitHub account. The `openssl` on a Mac is often LibreSSL, which can't
  make this key: run `openssl version` to check.
- Cluster nodes that can reach `ghcr.io`. Purlis's images are public, so pulling them needs no
  sign-in and no pull secret.
- purlis on your machine, for the last step.

Get the files the steps use, at the commit of the newest operator image, and run everything after
this from that folder:

_Not yet run in CI: CI starts from a checkout already._

```bash
git clone https://github.com/purlis/purlis.git
cd purlis
commit=$(gh run list --repo purlis/purlis --workflow operator-image.yml --branch main --status success --limit 1 --json displayTitle --jq '.[0].displayTitle | split(" ") | last')
git checkout "$commit"
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

## 2. Check the images

Your nodes pull both images from `ghcr.io`. Check each one against its signature first, so you
install only what purlis's own workflows signed.

Update purlis first, because this takes the newest runner image of your update channel, and it has
to match your app's version or purlis won't sign your profiles in there for you. Set `channel` to
your channel, `latest` for stable or `dev` for dev.

_Not yet run in CI: kind runs images it builds, not the published ones these commands read._

```bash
channel=latest
case $channel in
  dev) manifest=https://github.com/purlis/purlis/releases/download/dev/dev.json ;;
  *) manifest=https://github.com/purlis/purlis/releases/latest/download/latest.json ;;
esac
runner=$(curl -fsSL "$manifest" | jq -er '.runnerImage.image')
operator_digest=$(docker buildx imagetools inspect "ghcr.io/purlis/purlis-cluster-operator:sha-$commit" --format '{{.Manifest.Digest}}')
operator=ghcr.io/purlis/purlis-cluster-operator@$operator_digest
cosign verify "$operator" \
  --certificate-identity https://github.com/purlis/purlis/.github/workflows/operator-image.yml@refs/heads/main \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com &&
  echo "operator checked: $operator"
```

You should see `operator checked:`. **If you don't, stop here** and don't install the image.

Now check the runner image. The command below is for the dev channel. On stable, use
`runner-image.yml@refs/tags/v<version>`, with your version, in place of
`runner-image.yml@refs/heads/main`.

_Not yet run in CI: the same reason._

```bash
cosign verify "$runner" \
  --certificate-identity https://github.com/purlis/purlis/.github/workflows/runner-image.yml@refs/heads/main \
  --certificate-github-workflow-repository purlis/purlis \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com &&
  echo "runner checked: $runner"
```

You should see `runner checked:`. **If you don't, stop here** and don't use the image. Each `checked`
line ends with the image's digest, which starts with `sha256:`. Keep both: the operator's digest is
for step 5, and the runner's is for step 6.

## 3. Install agent-sandbox

The operator is built on this project. This installs the exact release purlis is tested with.

<!-- kind: upstream -->

```bash
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6
kubectl -n agent-sandbox-system rollout status deployment/agent-sandbox-controller --timeout=180s
```

You should see `successfully rolled out`. Run the doctor from step 1 again. It should now end with a
`nothing missing` line. Carry on with this guide from there.

## 4. Make the signing key

The operator uses this key to vouch for each runner, so a desktop only links to the runner it
claimed. Make it once and keep it. Running this a second time fails because the Secret exists,
and that's what you want: replacing the key makes every desktop check the pool again. To change
it later, see [Rolling the signing key](cluster-operator.md#rolling-the-signing-key).

<!-- kind: key -->

```bash
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
key=$(openssl genpkey -algorithm ed25519) &&
  printf '%s\n' "$key" |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin
unset key
```

You should see `namespace/purlis-system created`, then `labeled`, then
`secret/purlis-cluster-operator-signing-key created`.

## 5. Install the operator

Put in the operator's digest from step 2. CI renders this command with `helm template`, with its
own image in place of the published one, and checks it matches the chart CI installs, but doesn't
run it.

<!-- kind: install -->

```bash
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system \
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
policy is in place. Put in the runner's digest from step 2. This guide calls the pool `team`
throughout, so if you choose another name, change it in every command after this.

<!-- kind: pool -->

```bash
kubectl apply -f - <<'EOF'
apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team
spec:
  image: ghcr.io/purlis/purlis-runner@<runner digest>
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

1. Open Settings › You › This machine › Pools and press **Add a pool…**, or choose **Add a pool…** in the
   palette.
2. Set **Kind** to Cluster.
3. Give it a **Name**, leave **Kubeconfig** empty unless yours is somewhere unusual, choose the
   **Context** and type `team` as the **Pool**.
4. Press **Check its key**. The key shown must be the one step 6 printed. If it isn't, stop: you
   may be talking to another cluster.
5. Press **Add pool**. Its runner is claimed and paired, which can take a few minutes while the
   node pulls the image.
6. Sign your profiles in when the dialog offers it. A chat on the runner needs a profile signed in
   there.

The pool now shows in Settings › You › This machine › Pools, and new chats can run on it.

## Let a teammate use the pool

Each person needs the right to use the pool, and the right to make Claims in a namespace of their
own. Don't share one: anyone with these rights in a namespace can delete or stop the others' Claims
there, and their runners with them. Their kubeconfig's context has to name that namespace.

Put in the name the cluster knows them by, which `kubectl auth whoami` prints for them, and their
namespace.

<!-- kind: teammate -->

```bash
kubectl create namespace <their namespace>
kubectl apply -f - <<'EOF'
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: purlis-pool-team
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [pools]
    resourceNames: [team]
    verbs: [get, use]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: purlis-pool-team-<their namespace>
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: purlis-pool-team
subjects:
  - apiGroup: rbac.authorization.k8s.io
    kind: User
    name: <their name>
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: makes-claims
  namespace: <their namespace>
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [claims]
    verbs: [create, get, patch, delete]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: makes-claims
  namespace: <their namespace>
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: makes-claims
subjects:
  - apiGroup: rbac.authorization.k8s.io
    kind: User
    name: <their name>
EOF
kubectl auth can-i use pools.runners.purlis.dev/team --as <their name>
kubectl auth can-i get pools.runners.purlis.dev/team --as <their name>
kubectl auth can-i create claims.runners.purlis.dev --namespace <their namespace> --as <their name>
```

You should see `yes` three times at the end. Give them the pool's key too, so they can check it in step 7.
They only need step 7, and the operator gives them a namespace of their own for their runners.

## If something goes wrong

- **Check its key says `kubectl` failed:** read kubectl's words after it. If they say the server
  has no such resource, the operator isn't installed on that context's cluster. If they say the
  pool isn't found, the Pool has another name: check with `kubectl get pools`.
- **The add waits and then gives up:** run `kubectl describe claims -A`. The `Ready` condition says
  why the runner isn't up, for example `Pulling` with the kubelet's words when the node can't pull.
  A cluster with no default storage class stops here too.
- **Your nodes can't reach `ghcr.io`:** copy both images to a registry they can pull from with no
  pull secret, since a runner's pod can't name one. Sign in to your registry with `docker login`,
  then run `docker buildx imagetools create --tag <your registry>/purlis-runner:guide "$runner"`,
  and the same for `$operator`, which keeps each digest. Then add
  `--set image.repository=<your registry>/purlis-cluster-operator` in step 5, and name your registry
  in step 6.
- **Anything else:** run the doctor from step 1 again.
