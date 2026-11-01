// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #4469). Every step and state of Pool setup, on stand-in data. The
 * buttons move this page and nothing else: no Docker, cluster or forge is called.
 */
import { useEffect, useState, type ReactNode } from "react";
import { Box, Boxes, Container, MessageSquarePlus, Server } from "lucide-react";
import { Badge } from "../../ui/badge/Badge";
import { Button, ButtonRow } from "../../ui/button/Button";
import { Card } from "../../ui/card/Card";
import { Fact, Facts } from "../../ui/card/Facts";
import { Checkbox } from "../../ui/choice/Checkbox";
import { OptionCard, OptionCards, Radio, RadioGroup } from "../../ui/choice/Radio";
import { TextField, type TextFieldProps } from "../../ui/field/Field";
import { Meter, ProgressBar } from "../../ui/meter/Meter";
import { Select } from "../../ui/select/Select";
import { STEP_NAME, STEPS, type Sim, type StepId, type StepState } from "./data";
import {
  Action,
  Busy,
  CheckLine,
  Checks,
  Code,
  Options,
  Out,
  Rechecking,
  Recommended,
  Say,
  SeeWhat,
  StepHead,
  Term,
  Terms,
} from "./kit";

export type Api = {
  sim: Sim;
  /** Show another screen of this step, and set its state and line if given. */
  show: (screen: string, state?: StepState, line?: string) => void;
  /** This step is done, saying `line`; on to the next. */
  done: (line?: string, nextScreen?: string) => void;
  /** Skip for now; on to the next. */
  skip: (line?: string) => void;
  /** Go to a step. */
  go: (step: StepId, screen?: string) => void;
  /** Change anything. */
  patch: (to: (sim: Sim) => Sim) => void;
  /** Leave the tab for a screen around it. */
  around: (id: string | undefined) => void;
  /** Tell the person this prototype stops here. */
  say: (words: string) => void;
};

/** Moves on after a moment, as the real check would. Only while this screen is showing. */
function useAfter(ms: number, then: () => void) {
  useEffect(() => {
    const later = setTimeout(then, ms);
    return () => clearTimeout(later);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

const GetHelp = ({ api }: { api: Api }) => (
  <Button variant="ghost" onPress={() => api.show("help")}>
    Get help
  </Button>
);
const SkipFor = ({ api, line }: { api: Api; line?: string }) => (
  <Button variant="ghost" onPress={() => api.skip(line)}>
    Skip for now
  </Button>
);
const stepOf = (api: Api) => {
  const order = STEPS[api.sim.kind];
  return { n: order.indexOf(api.sim.at) + 1, of: order.length };
};

/** A box with its words already in it. */
function Held({ defaultValue, ...rest }: Omit<TextFieldProps, "value" | "onChange"> & { defaultValue: string }) {
  const [value, setValue] = useState(defaultValue);
  const given = { ...rest, value, onChange: setValue } as unknown as TextFieldProps;
  return <TextField {...given} />;
}

/* ------------------------------ Where ------------------------------ */

const DOCKER_CARD =
  "Runs here, on your Mac. Good for trying things and for working alone. Chats stop when this Mac sleeps.";
const CLUSTER_CARD =
  "Runs on the computers your team already has. Chats keep going when your Mac is closed. Needs a Kubernetes cluster.";

function WhereTerms() {
  return (
    <Terms>
      <Term word="A pool">is the place your chats run instead of directly on this Mac.</Term>
      <Term word="A runner">
        is one separate machine in that pool. A chat there can only touch what you send to it.
      </Term>
    </Terms>
  );
}

function WhereCards({
  value,
  onChange,
  docker,
  cluster,
  dockerTag,
  clusterTag,
}: {
  value: string;
  onChange: (to: string) => void;
  docker: ReactNode;
  cluster: ReactNode;
  dockerTag?: ReactNode;
  clusterTag?: ReactNode;
}) {
  return (
    <OptionCards aria-label="Where should chats run?" value={value} onChange={onChange}>
      <OptionCard
        value="docker"
        title={<span className="ps-card-title">On this Mac, in Docker {dockerTag}</span>}
        icon={<Container aria-hidden size={20} />}
        detail={
          <>
            <span className="ps-card-detail">{DOCKER_CARD}</span>
            <span className="ps-card-found">{docker}</span>
          </>
        }
      />
      <OptionCard
        value="cluster"
        title={<span className="ps-card-title">On your team's cluster {clusterTag}</span>}
        icon={<Boxes aria-hidden size={20} />}
        detail={
          <>
            <span className="ps-card-detail">{CLUSTER_CARD}</span>
            <span className="ps-card-found">{cluster}</span>
          </>
        }
      />
    </OptionCards>
  );
}

const WHERE_SEE = `# Only reads. Nothing changes on this Mac.
docker version --format '{{.Server.Version}}'
kubectl config current-context
purlis pool doctor`;

function Where({ api, screen }: { api: Api; screen: string }) {
  const [pick, setPick] = useState(
    ["cluster-ok", "no-cluster", "team"].includes(screen) ? "cluster" : "docker",
  );
  const toCluster = () => {
    setPick("cluster");
    api.patch((sim) => ({ ...sim, kind: "cluster", screens: { ...sim.screens, where: "cluster-ok" } }));
  };
  const toDocker = (to: string) => {
    setPick("docker");
    api.patch((sim) => ({
      ...sim,
      kind: "docker",
      states: { ...sim.states, where: to === "docker-ok" ? undefined : sim.states.where },
      screens: { ...sim.screens, where: to },
    }));
  };
  useAfter(1500, () => {
    if (screen === "checking") api.show("docker-ok");
  });

  const title = "Where should your chats run?";
  const lead =
    "Right now your chats run directly on this Mac. A pool gives each chat a separate machine, so it can work on its own.";

  if (screen === "checking")
    return (
      <>
        <StepHead title={title} lead={lead} />
        <WhereTerms />
        <Busy>Looking at what this Mac has. This only reads.</Busy>
      </>
    );

  const dockerState: Record<string, ReactNode> = {
    "docker-ok": "Found: Docker 28.1 is running.",
    "docker-found": "Found: Docker 28.1 is running.",
    "docker-missing": "Not on this Mac yet.",
    "docker-stopped": "Installed, but not running.",
  };
  const clusterSaid =
    screen === "team"
      ? "Found: team-prod. Your team already has 2 pools there."
      : screen === "no-cluster"
        ? "No cluster found on this Mac."
        : screen === "docker-missing" || screen === "docker-stopped"
          ? "No cluster found on this Mac."
          : "Found: team-prod, in your Kubernetes settings.";

  const dockerGood = screen === "docker-ok" || screen === "docker-found";
  const onPick = (to: string) => {
    if (to === "cluster") {
      if (pick !== "cluster") toCluster();
    } else toDocker("docker-ok");
  };

  return (
    <>
      <StepHead title={title} lead={lead} />
      <WhereTerms />
      <WhereCards
        value={pick}
        onChange={onPick}
        docker={dockerState[screen] ?? "Found: Docker 28.1 is running."}
        cluster={clusterSaid}
        dockerTag={
          pick === "docker" && screen !== "team" ? <Recommended /> : undefined
        }
        clusterTag={screen === "team" ? <Recommended /> : undefined}
      />

      {dockerGood && (
        <p className="ps-why">
          <strong>Why Docker:</strong> it is already running on this Mac, so you can have a working
          chat in a few minutes. You can add a cluster later.
        </p>
      )}
      {screen === "docker-found" && (
        <Say tone="news" title="Docker is ready">
          purlis noticed it started. You didn't need to do anything here.
        </Say>
      )}
      {screen === "team" && (
        <p className="ps-why">
          <strong>Why your team's cluster:</strong> your team has already set it up, so you only
          need your own sign-ins. This takes a couple of minutes.
        </p>
      )}

      {screen === "docker-missing" && (
        <Card>
          <h2 className="ps-h2">You need Docker first</h2>
          <p className="ps-p">
            <strong>Docker</strong> is a free app that runs separate small machines on your Mac.
            purlis uses it to make your pool. Docker's own page installs it. It takes about five
            minutes, and you can leave this tab open.
          </p>
          <Rechecking>Checking again by itself every few seconds. Last checked just now.</Rechecking>
        </Card>
      )}
      {screen === "docker-stopped" && (
        <Card>
          <h2 className="ps-h2">Docker needs to be running</h2>
          <p className="ps-p">
            Docker is installed but isn't open. Open it and wait for its whale icon to appear in
            your menu bar.
          </p>
          <Rechecking>Checking again by itself every few seconds. Last checked just now.</Rechecking>
        </Card>
      )}
      {screen === "no-cluster" && (
        <Say
          tone="attention"
          title="There's no cluster on this Mac"
          actions={
            <>
              <Button onPress={() => toDocker("docker-ok")}>Use Docker instead</Button>
              <Button variant="ghost" onPress={() => api.show("cluster-ok")}>
                Check again
              </Button>
            </>
          }
        >
          A <strong>cluster</strong> is a group of computers your team runs with Kubernetes. purlis
          looked in your Kubernetes settings and found none. Docker works without one, and you can
          come back if your team gives you access.
        </Say>
      )}
      {screen === "cluster-ok" && (
        <Terms>
          <Term word="A cluster">
            is a group of computers your team runs with Kubernetes. purlis uses your own access to
            it and stores none of it.
          </Term>
        </Terms>
      )}

      <Options>
        {pick === "cluster" ? (
          <>
            <Select
              aria-label="Which cluster"
              value="team-prod"
              onValueChange={() => undefined}
              options={[
                { value: "team-prod", label: "team-prod (current)" },
                { value: "kind-team", label: "kind-team" },
              ]}
            />
            <Held label="Kubernetes settings file" defaultValue="" placeholder="~/.kube/config" help="Leave empty to use the usual place." />
          </>
        ) : (
          <Held
            label="Docker address"
            defaultValue=""
            placeholder="unix:///var/run/docker.sock"
            help="Leave empty to use the one Docker set up. Recommended."
          />
        )}
      </Options>
      <SeeWhat
        text={WHERE_SEE}
        note="This step only looks. It saves your choice and changes nothing."
      />

      {screen === "docker-missing" && (
        <Action
          does="Opens docker.com in your browser, on the download for your Mac (Apple chip). purlis doesn't install anything."
          main="Open Docker's install page"
          mainIcon={<Out />}
          onMain={() => api.say("Would open Docker's own install page in your browser.")}
        />
      )}
      {screen === "docker-stopped" && (
        <Action
          does="Opens the Docker app on this Mac."
          main="Open Docker"
          onMain={() => api.show("docker-found", "current")}
        />
      )}
      {dockerGood && (
        <Action
          does="Saves your choice. Nothing on your Mac changes yet."
          main="Continue with Docker"
          onMain={() => api.done("Docker on this Mac", "form")}
        />
      )}
      {screen === "cluster-ok" && (
        <Action
          does="Saves your choice. Nothing on the cluster changes yet."
          main="Continue with team-prod"
          onMain={() => api.done("Cluster team-prod", "ok")}
        />
      )}
      {screen === "team" && (
        <Action
          does="Saves your choice. You'll see your team's pools next. Nothing on the cluster changes."
          main="See your team's pools"
          onMain={() =>
            api.patch((sim) => ({
              ...sim,
              kind: "join",
              at: "pool",
              screens: { ...sim.screens, pool: "team-list" },
              states: { where: "done", doctor: "done", install: "done" },
              lines: {
                where: "Cluster team-prod",
                doctor: "Found: nothing missing",
                install: "Found: operator 0.9.2",
              },
            }))
          }
        />
      )}
    </>
  );
}

/* ------------------------------ Docker: Pool ------------------------------ */

const POOL_YAML = `# 1. Download the runner image, named by its digest so it can't change under you.
docker pull ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2…

# 2. Add the pool. purlis makes its runner and pairs it with this Mac.
purlis pool add dev-box --kind docker \\
  --image ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2…`;

function PoolOptions({ imageBad, open }: { imageBad?: boolean; open?: boolean }) {
  const [image, setImage] = useState(imageBad ? "team" : "own");
  return (
    <Options open={open || imageBad}>
      <RadioGroup aria-label="Runner image" value={image} onChange={setImage}>
        <Radio
          value="own"
          description="A ready-made image with git, gh and glab. Named by its digest: sha256:7be1c4f0a2…"
        >
          purlis's image, this version <Recommended />
        </Radio>
        <Radio value="team" description="An image your team built that keeps the runner image contract.">
          My team's own image
        </Radio>
      </RadioGroup>
      {image === "team" && (
        <Held
          label="Image, by digest"
          defaultValue="registry.acme.dev/runner@sha256:40ad91…"
          error={imageBad ? "This image doesn't keep the contract. The problems are listed above." : undefined}
        />
      )}
      <h3 className="ps-h3">Limits</h3>
      <div className="ps-grid">
        <Held label="Largest code sent" defaultValue="" placeholder="1024" help="MiB. Recommended: 1024." />
        <Held label="Most a profile may send" defaultValue="" placeholder="128" help="MiB. Recommended: 128." />
        <Held label="Memory" defaultValue="" placeholder="4096" help="MiB. Recommended: 4096." />
        <Held label="Processors" defaultValue="" placeholder="2" help="Recommended: 2." />
      </div>
      <Held
        label="Suspend after no link for"
        defaultValue=""
        placeholder="30"
        help="Minutes the runner waits for this Mac before it pauses. Recommended: 30."
      />
    </Options>
  );
}

function PoolStep({ api, screen }: { api: Api; screen: string }) {
  const [name, setName] = useState(screen === "taken" ? "dev-box" : "dev-box");
  useAfter(2600, () => {
    if (screen === "creating") api.done("dev-box", "list");
  });
  const title = "Name your pool";
  const lead = "This is the name you'll see when you choose where a chat runs. The rest is already chosen for you.";
  const create = (
    <Action
      does="Downloads the runner image (about 530 MB) and starts one runner in Docker. Nothing else on your Mac changes."
      main={`Create pool ${name}`}
      onMain={() => api.show("creating", "current")}
    >
      <SkipLater />
    </Action>
  );

  if (screen === "creating")
    return (
      <>
        <StepHead title="Creating your pool" lead="You can keep working or close this tab. Setup picks up here when you come back." />
        <Card>
          <div className="ps-stack">
            <ProgressBar label="Downloading the runner image" value={62} valueLabel="330 of 530 MB" />
            <Checks label="Creating the pool">
              <CheckLine verdict="ok" title="Docker is running" />
              <CheckLine verdict="running" title="Download the runner image" />
              <CheckLine verdict="todo" title="Check the image is the one purlis names" />
              <CheckLine verdict="todo" title="Start a runner and pair it with this Mac" />
            </Checks>
          </div>
        </Card>
      </>
    );

  if (screen === "failed" || screen === "help")
    return (
      <>
        <StepHead title="Your pool wasn't created" lead="Only this step stopped. Your earlier choices are saved." />
        <Say
          tone="trouble"
          title="Docker stopped while the image was downloading"
          actions={
            <>
              <Button variant="primary" onPress={() => api.show("creating", "current")}>
                Open Docker and try again
              </Button>
              <GetHelp api={api} />
            </>
          }
        >
          The download stopped at 330 of 530 MB. What's already downloaded is kept. Docker needs to
          be running before this can go on.
        </Say>
        {screen === "help" && (
          <Card>
            <h2 className="ps-h2">Get help opens a chat with this</h2>
            <p className="ps-p">
              The chat can explain what went wrong. It can't change anything, so any fix still goes
              through this step's button.
            </p>
            <Code
              label="What the chat is told"
              copy="Copy"
              text={`Pool setup, step 2 of 5: Pool (Docker)
purlis 0.42.0, Docker 28.1, macOS 26.2
What happened: Docker stopped while the image was downloading
docker pull ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2…
error: Cannot connect to the Docker daemon at unix:///var/run/docker.sock`}
            />
            <p className="ps-note">It holds no secrets, tokens, keys or cluster credentials.</p>
            <ButtonRow size="md">
              <Button
                variant="primary"
                icon={<MessageSquarePlus aria-hidden size={14} />}
                onPress={() => api.say("Would open a new chat holding these lines.")}
              >
                Open the help chat
              </Button>
            </ButtonRow>
          </Card>
        )}
        <SeeWhat text={POOL_YAML} />
      </>
    );

  if (screen === "registry")
    return (
      <>
        <StepHead title="One sign-in before the download" lead="Only this step stopped. Your pool's name and choices are saved." />
        <Say
          tone="attention"
          title="GitHub needs to know it's you"
          actions={
            <>
              <Button variant="primary" icon={<Out />} onPress={() => api.show("creating", "current")}>
                Sign in to GitHub
              </Button>
              <GetHelp api={api} />
            </>
          }
        >
          purlis's runner image is kept on GitHub, and downloading it takes a GitHub sign-in with
          access to it. This sign-in is used only by this Mac's Docker. It never reaches a chat.
        </Say>
        <SeeWhat text={`docker login ghcr.io\n${POOL_YAML}`} />
      </>
    );

  if (screen === "by-hand")
    return (
      <>
        <StepHead title="Your pool is ready" tag={<Badge variant="ok">Done</Badge>} lead="You ran this one yourself, and purlis noticed." />
        <Say tone="news" title="Found pool dev-box">
          It was added with <code>purlis pool add</code> 2 minutes ago, and its runner is running.
          Your setup follows what you did.
        </Say>
        <Action main="Continue to sign-ins" onMain={() => api.done("dev-box", "list")} />
      </>
    );

  if (screen === "change")
    return (
      <>
        <StepHead title="Pool dev-box" tag={<Badge variant="ok">Done</Badge>} lead="Change anything here. A change that needs a new runner says so before it happens." />
        <Facts aria-label="About this pool">
          <Fact label="Runner">Running, 2 chats on it</Fact>
          <Fact label="Image">purlis's image, 0.42.0</Fact>
        </Facts>
        <Say tone="attention" title="A new memory limit needs a new runner">
          New chats start on the new runner at once. Your 2 running chats stay on the old runner
          until they end, then it's removed. Sign-ins carry over when it goes, and any that can't
          will ask you to sign in again.
        </Say>
        <PoolOptions open />
        <Action
          does="Starts a new runner beside the old one with 8192 MiB of memory. Running chats aren't interrupted."
          main="Apply and start a new runner"
          onMain={() => api.say("Would start a new runner beside the old one.")}
        >
          <Button variant="ghost" onPress={() => api.say("Would put the limit back.")}>
            Keep it as it was
          </Button>
        </Action>
      </>
    );

  if (screen === "image-bad")
    return (
      <>
        <StepHead title={title} lead="Your team's image can't be used yet. Nothing was created." />
        <Say tone="trouble" title="This image is missing 2 things a runner needs" actions={
          <>
            <Button onPress={() => api.show("form", "current")}>Use purlis's image instead</Button>
            <GetHelp api={api} />
          </>
        }>
          Ask whoever builds it to fix these, or use purlis's image for now.
        </Say>
        <Checks label="The contract check">
          <CheckLine verdict="ok" title="Linux, with a shell at /bin/sh" />
          <CheckLine verdict="missing" title="A user who isn't root">
            The image runs as root. Add <code>USER 10001:10001</code>.
          </CheckLine>
          <CheckLine verdict="missing" title="purlis's plugin">
            Nothing at <code>/usr/local/lib/purlis/plugin</code>.
          </CheckLine>
          <CheckLine verdict="ok" title="The purlis program answers" />
        </Checks>
        <PoolOptions imageBad />
        <SeeWhat
          text={`node tools/runner-image-contract.mjs registry.acme.dev/runner@sha256:40ad91…`}
          note="This only reads the image. Your team can run it on their own build."
        />
      </>
    );

  const taken = screen === "taken";
  return (
    <>
      <StepHead title={title} lead={lead} />
      <div className="ps-field">
        <TextField
          label="Pool name"
          value={name}
          onChange={setName}
          error={taken && name === "dev-box" ? "You already have a pool called dev-box. Try dev-box-2." : undefined}
          help="Letters, numbers and dashes."
        />
      </div>
      {taken && (
        <p className="ps-note">Nothing was created. Pick another name and press the button again.</p>
      )}
      <Facts aria-label="What purlis chose for you">
        <Fact label="Image">purlis's image, this version</Fact>
        <Fact label="Limits">Recommended, for one person's chats</Fact>
      </Facts>
      <PoolOptions open={screen === "form-open"} />
      <SeeWhat text={POOL_YAML} open={screen === "form-open"} />
      {create}
    </>
  );
}

/** Pool can't be skipped: nothing after it works without a pool. */
const SkipLater = () => null;

/* ------------------------------ Cluster: Check ------------------------------ */

const DOCTOR = `purlis pool doctor --context team-prod

ok: asked context team-prod, API server https://k8s.acme.dev:6443/
ok: Kubernetes v1.36.4
ok: you may apply the install's cluster-scoped definitions
missing: upstream agent-sandbox v1.0.6: not installed.
1 missing: the Install step puts this in`;

const ADMIN_MESSAGE = `Hi, I'd like to run purlis chats on our team-prod cluster and I don't have the rights to install what it needs. Could you run this, or give me the rights?

What's needed
- Kubernetes v1.36 or later (found v1.36.4)
- agent-sandbox v1.0.6 (not installed)
- purlis cluster operator 0.9.2 (not installed)
- Rights to apply namespaces, definitions, cluster roles and admission policies

Commands
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6
kubectl apply --server-side -f operator/deploy/crds.yaml
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \\
    --from-file=key.pem=/dev/stdin
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \\
  --namespace purlis-system --set image.digest=sha256:91c0e2…

The signing key is made in your cluster and stays there. Don't send it to me.
My setup will notice when this is done.`;

function SendToAdmin({ api }: { api: Api }) {
  return (
    <>
      <StepHead
        title="Send to your admin"
        lead="This holds what's needed and the exact commands. Send it however your team talks: chat, email or a ticket."
      />
      <Code label="Your message" text={ADMIN_MESSAGE} copy="Copy text" wraps />
      <Card>
        <h2 className="ps-h2">What's in it, and what isn't</h2>
        <p className="ps-p">
          It holds step names, version numbers, what purlis found and commands. It has no secrets,
          tokens, keys or cluster credentials, so it's safe to paste anywhere.
        </p>
      </Card>
      <Action
        does="Marks this step as waiting and starts checking the cluster by itself. Sends nothing: you send the message."
        main="I've sent it"
        onMain={() =>
          api.patch((sim) => ({
            ...sim,
            screens: { ...sim.screens, doctor: "admin-wait" },
            states: { ...sim.states, doctor: "waiting", install: "waiting" },
            lines: { ...sim.lines, doctor: "Waiting for your admin", install: "Waiting for your admin" },
          }))
        }
      >
        <Button icon={<Out />} variant="secondary" onPress={() => api.say("Would copy a link that opens the same text, with nothing secret in it.")}>
          Copy a link instead
        </Button>
        <Button variant="ghost" onPress={() => api.show("no-rights", "attention")}>
          Back
        </Button>
      </Action>
    </>
  );
}

function Doctor({ api, screen }: { api: Api; screen: string }) {
  useAfter(1800, () => {
    if (screen === "running") api.show("ok", "current");
  });
  const lead = "purlis asks the cluster what it has and what you may do. This only reads, so nothing changes.";
  if (screen === "send") return <SendToAdmin api={api} />;
  if (screen === "running")
    return (
      <>
        <StepHead title="Checking the cluster" lead={lead} />
        <Checks label="What the check finds">
          <CheckLine verdict="ok" title="Reached team-prod" />
          <CheckLine verdict="running" title="Kubernetes version" />
          <CheckLine verdict="todo" title="What you're allowed to install" />
          <CheckLine verdict="todo" title="What's already there" />
        </Checks>
      </>
    );
  if (screen === "unreachable")
    return (
      <>
        <StepHead title="Couldn't reach the cluster" lead="Only this step stopped. Nothing was changed." />
        <Say
          tone="trouble"
          title="team-prod didn't answer"
          actions={
            <>
              <Button variant="primary" onPress={() => api.show("running", "current")}>
                Check again
              </Button>
              <Button variant="secondary" onPress={() => api.go("where", "cluster-ok")}>
                Choose another cluster
              </Button>
              <GetHelp api={api} />
            </>
          }
        >
          The address k8s.acme.dev:6443 didn't answer after 10 seconds. If your team uses a VPN,
          turn it on and check again.
        </Say>
        <SeeWhat text="kubectl --context team-prod version" note="This only reads. Try it in your own terminal to see the same answer." />
      </>
    );
  if (screen === "admin-wait")
    return (
      <>
        <StepHead
          title="Waiting for your admin"
          tag={<Badge variant="waiting">Waiting</Badge>}
          lead="You can close this tab and keep working. Your other chats aren't affected."
        />
        <Card>
          <Rechecking>Checking team-prod by itself every minute. Last checked 20 seconds ago.</Rechecking>
          <p className="ps-p">
            When the install is there, you get one entry in Needs you, and setup carries on from
            the next step.
          </p>
        </Card>
        <Checks label="What's still missing">
          <CheckLine verdict="missing" title="agent-sandbox v1.0.6" />
          <CheckLine verdict="missing" title="purlis cluster operator" />
        </Checks>
        <Action>
          <Button onPress={() => api.show("send", "attention")}>See the message again</Button>
          <Button variant="ghost" onPress={() => api.go("where", "docker-ok")}>
            Use Docker in the meantime
          </Button>
        </Action>
      </>
    );
  if (screen === "admin-ready")
    return (
      <>
        <StepHead title="Your admin finished" tag={<Badge variant="ok">Done</Badge>} lead="purlis noticed the install on team-prod. You can carry on." />
        <Checks label="What was found">
          <CheckLine verdict="ok" title="Kubernetes v1.36.4" />
          <CheckLine verdict="ok" title="agent-sandbox v1.0.6" />
          <CheckLine verdict="ok" title="purlis cluster operator 0.9.2" />
          <CheckLine verdict="ok" title="A signing key, kept as it is" />
        </Checks>
        <Action main="Continue to your pool" onMain={() => api.go("pool", "team-list")} />
      </>
    );
  if (screen === "found")
    return (
      <>
        <StepHead title="Check the cluster" tag={<Badge variant="ok">Found done</Badge>} lead="Your team has already set this up, so there's nothing for you to do here." />
        <Checks label="What was found">
          <CheckLine verdict="ok" title="Kubernetes v1.36.4" />
          <CheckLine verdict="ok" title="agent-sandbox v1.0.6" />
          <CheckLine verdict="ok" title="purlis cluster operator 0.9.2" />
        </Checks>
        <Action main="Back to your team's pools" onMain={() => api.go("pool", "team-list")} />
      </>
    );
  if (screen === "old")
    return (
      <>
        <StepHead title="This cluster can't take the install yet" lead="Nothing was changed. One thing has to be upgraded first." />
        <Checks label="What was found">
          <CheckLine verdict="ok" title="Reached team-prod" />
          <CheckLine verdict="missing" title="Kubernetes v1.34.2">
            purlis needs v1.36 or later. Your admin can upgrade it.
          </CheckLine>
          <CheckLine verdict="ok" title="You may install what's needed" />
        </Checks>
        <Action
          does="Shows a message for your admin with what's needed. Nothing is sent by itself."
          main="Send to your admin"
          onMain={() => api.show("send", "attention")}
        >
          <Button variant="secondary" onPress={() => api.show("running", "current")}>
            Check again
          </Button>
          <Button variant="ghost" onPress={() => api.go("where", "docker-ok")}>
            Use Docker instead
          </Button>
        </Action>
        <SeeWhat text="purlis pool doctor --context team-prod" note="This only reads. Run it yourself for the same answer." />
      </>
    );
  if (screen === "no-rights")
    return (
      <>
        <StepHead title="You'll need your admin for the next part" lead="The cluster is fine. Your account just isn't allowed to install things on it, and that's normal on a team." />
        <Checks label="What was found">
          <CheckLine verdict="ok" title="Reached team-prod" />
          <CheckLine verdict="ok" title="Kubernetes v1.36.4" />
          <CheckLine verdict="missing" title="Rights to install">
            Your account can't create cluster roles or definitions.
          </CheckLine>
          <CheckLine verdict="missing" title="agent-sandbox v1.0.6: not installed" />
        </Checks>
        <Action
          does="Shows a message for your admin with what's needed and the commands. You decide how to send it."
          main="Send to your admin"
          onMain={() => api.show("send", "attention")}
        >
          <Button variant="ghost" onPress={() => api.go("where", "docker-ok")}>
            Use Docker instead
          </Button>
        </Action>
      </>
    );
  return (
    <>
      <StepHead title="Nothing stops the install" lead={lead} />
      <Terms>
        <Term word="agent-sandbox">
          is an open project that keeps each runner separate on a cluster. The next step installs it.
        </Term>
      </Terms>
      <Checks label="What was found">
        <CheckLine verdict="ok" title="Reached team-prod" trail="k8s.acme.dev" />
        <CheckLine verdict="ok" title="Kubernetes v1.36.4" />
        <CheckLine verdict="ok" title="You may install what's needed" />
        <CheckLine verdict="todo" title="agent-sandbox isn't installed yet">
          That's expected. The next step puts it in.
        </CheckLine>
      </Checks>
      <Options>
        <Checkbox>Check again when I come back to this step</Checkbox>
      </Options>
      <SeeWhat text={DOCTOR} label="The command and its answer" note="This step only reads. Run it yourself for the same answer." />
      <Action does="Moves to the install. Nothing on the cluster has changed." main="Continue to install" onMain={() => api.done("Nothing stopping it", "i0")} />
    </>
  );
}

/* ------------------------------ Cluster: Install ------------------------------ */

const INSTALL = [
  {
    name: "Upstream agent-sandbox",
    what: "Installs agent-sandbox v1.0.6, which keeps each runner separate.",
    button: "Install agent-sandbox",
    found: "agent-sandbox v1.0.6 is running",
    cmd: "kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6",
  },
  {
    name: "purlis's definitions",
    what: "Teaches the cluster what a Pool and a Claim are, and adds the policy on Claims.",
    button: "Add the definitions",
    found: "Pool and Claim are defined",
    cmd: "kubectl apply --server-side -f operator/deploy/policy.yaml\nkubectl apply --server-side -f operator/deploy/crds.yaml",
  },
  {
    name: "Signing key",
    what: "Makes a key inside the cluster so desktops can tell a runner is really yours. It never leaves the cluster.",
    button: "Make the signing key",
    found: "Made in the cluster, and it stays there",
    cmd: `kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \\
    --from-file=key.pem=/dev/stdin`,
  },
  {
    name: "Operator",
    what: "Starts the purlis operator, which makes a runner whenever someone asks for one.",
    button: "Install the operator",
    found: "Operator 0.9.2 is running",
    cmd: `helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \\
  --namespace purlis-system --skip-crds \\
  --set image.digest=sha256:91c0e2… \\
  --set 'imagePullSecrets[0].name=purlis-pull'`,
  },
];

function Install({ api, screen }: { api: Api; screen: string }) {
  const failed = screen === "op-failed";
  const kept = screen === "key-kept";
  const at = failed ? 3 : kept ? 3 : Number(screen.slice(1)) || 0;
  const all = at >= 4;
  return (
    <>
      <StepHead
        title={all ? "Installed on team-prod" : "Install on team-prod"}
        lead={
          all
            ? "Everything a pool needs is on the cluster."
            : "Four parts, one button each. Nothing changes until you press. purlis uses your own cluster access and stores none of it."
        }
      />
      <ol className="ps-install" aria-label="The four parts of the install">
        {INSTALL.map((part, i) => {
          const isDone = i < at;
          const now = i === at;
          return (
            <li key={part.name} className="ps-install-part" data-now={now ? "" : undefined}>
              <CheckLineBare
                verdict={isDone ? "ok" : failed && now ? "missing" : now ? "running-none" : "todo"}
              />
              <div className="ps-install-words">
                <h2 className="ps-h3">
                  {i + 1}. {part.name}
                </h2>
                <p className="ps-p">
                  {kept && i === 2
                    ? "Already here, from 12 March. It's kept, so nobody's desktop is asked to trust the cluster again."
                    : isDone
                      ? part.found
                      : part.what}
                </p>
                {now && failed && (
                  <Say
                    tone="trouble"
                    title="The operator's image couldn't be downloaded"
                    actions={
                      <>
                        <Button variant="primary" onPress={() => api.show("i4", "current", "Operator 0.9.2 running")}>
                          Try again
                        </Button>
                        <GetHelp api={api} />
                      </>
                    }
                  >
                    The cluster was refused by ghcr.io. It needs a secret that can download purlis's
                    image: name one under See what this does, then try again. Parts 1 to 3 are done
                    and stay done.
                  </Say>
                )}
                {now && !failed && (
                  <>
                    {i === 3 && (
                      <div className="ps-field">
                        <Held
                          label="Secret that downloads purlis's image"
                          defaultValue="purlis-pull"
                          help="A Kubernetes image pull secret in purlis-system. It's only named here: purlis doesn't read it."
                        />
                      </div>
                    )}
                    <ButtonRow size="lg">
                      <Button variant="primary" onPress={() => api.show(`i${i + 1}`, "current")}>
                        {part.button}
                      </Button>
                    </ButtonRow>
                  </>
                )}
                <SeeWhat text={part.cmd} />
              </div>
            </li>
          );
        })}
      </ol>
      <Options>
        <Held label="Namespace" defaultValue="purlis-system" help="Where the operator and its signing key live. Recommended: purlis-system." />
        <Held label="Operator image digest" defaultValue="sha256:91c0e2…" help="The image this version of purlis names. Recommended." />
        <Select
          aria-label="How it's installed"
          value="helm"
          onValueChange={() => undefined}
          options={[
            { value: "helm", label: "Install with the Helm chart. Recommended" },
            { value: "plain", label: "Install with plain manifests" },
          ]}
        />
      </Options>
      {all && (
        <Action main="Continue to your pool" does="Moves to the next step." onMain={() => api.done("Operator 0.9.2 running", "cluster-form")} />
      )}
      {!all && (
        <p className="ps-note">
          Removing this later is never a button here. A pool's page gives the exact commands, and how
          many people it affects.
        </p>
      )}
    </>
  );
}

const CheckLineBare = ({ verdict }: { verdict: string }) => (
  <span className="ps-install-mark" data-verdict={verdict} aria-hidden>
    {verdict === "ok" ? "✓" : verdict === "missing" ? "!" : ""}
  </span>
);

/* ------------------------------ Cluster: Pool, joining ------------------------------ */

const CLUSTER_POOL = `apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team-gpu
spec:
  image: ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2…`;

function ClusterPool({ api, screen }: { api: Api; screen: string }) {
  if (screen === "team-list") return <TeamList api={api} />;
  if (screen === "joined")
    return (
      <>
        <StepHead title="You've joined team-gpu" tag={<Badge variant="ok">Done</Badge>} lead="Your team looks after this pool. You only need your own sign-ins and identity." />
        <Facts aria-label="About team-gpu">
          <Fact label="Looked after by">Sam, your admin</Fact>
          <Fact label="Your runner">Made for you alone, when you start your first chat</Fact>
          <Fact label="Signing key">
            <code>ed25519:Q3v9…x1Lk</code>. Your admin can confirm it.
          </Fact>
        </Facts>
        <Action main="Continue to your sign-ins" onMain={() => api.done("team-gpu", "claim")} />
      </>
    );
  return (
    <>
      <StepHead title="Create the first pool on team-prod" lead="People on your team will choose this pool to run their chats." />
      <div className="ps-field">
        <Held label="Pool name" defaultValue="team-gpu" help="Letters, numbers and dashes." />
      </div>
      <Options>
        <Held label="Image, by digest" defaultValue="ghcr.io/purlis/purlis-runner@sha256:7be1c4f0a2…" help="purlis's image, this version. Recommended." />
        <div className="ps-grid">
          <Held label="Memory per runner" defaultValue="" placeholder="4096" help="MiB. Recommended: 4096." />
          <Held label="Processors per runner" defaultValue="" placeholder="2" help="Recommended: 2." />
        </div>
      </Options>
      <SeeWhat text={`kubectl apply -f - <<EOF\n${CLUSTER_POOL}\nEOF`} />
      <Action does="Adds this Pool to team-prod, for everyone on your team to see." main="Create pool team-gpu" onMain={() => api.done("team-gpu", "claim")} />
    </>
  );
}

function TeamList({ api }: { api: Api }) {
  const [pool, setPool] = useState("team-gpu");
  return (
    <>
      <StepHead title="Your team has these pools" lead="Choose one to join. It's already set up, so there's nothing to install." />
      <RadioGroup aria-label="Your team's pools" value={pool} onChange={setPool}>
        <Radio value="team-gpu" description="Looked after by Sam, your admin. 8 people use it. Keeps chats going when your Mac is closed.">
          team-gpu <Recommended />
        </Radio>
        <Radio value="ci-small" description="Looked after by Sam, your admin. 3 people use it. A smaller runner.">
          ci-small
        </Radio>
      </RadioGroup>
      <p className="ps-note">Other people's sign-ins and chats aren't visible to you, and yours aren't visible to them.</p>
      <SeeWhat text={`purlis pool add ${pool} --kind cluster --context team-prod`} />
      <Action does="Adds this pool to your purlis and remembers its signing key. Nothing on the cluster changes until your first chat." main={`Join ${pool}`} onMain={() => api.show("joined", "current", pool)} />
    </>
  );
}

/* ------------------------------ Sign-ins ------------------------------ */

type Prof = { name: string; harness: string; used: string; mib: number; signed: boolean; recent: boolean };
const PROFILES: Prof[] = [
  { name: "work", harness: "Claude Code", used: "Used today", mib: 42, signed: true, recent: true },
  { name: "personal", harness: "Claude Code", used: "Used yesterday", mib: 18, signed: false, recent: true },
  { name: "oss", harness: "Codex", used: "Used 6 weeks ago", mib: 9, signed: false, recent: false },
];

function SignIns({ api, screen }: { api: Api; screen: string }) {
  const [ticks, setTicks] = useState<Record<string, boolean>>({ work: true, personal: true, oss: false });
  const over = screen === "over" && ticks.work;
  const limit = screen === "raised" ? 256 : 128;
  const profs = PROFILES.map((p) =>
    p.name === "work" && (screen === "over" || screen === "raised") ? { ...p, mib: 212 } : p,
  ).map((p) => (p.name === "personal" && screen === "done" ? { ...p, signed: true } : p));
  const needs = profs.filter((p) => ticks[p.name] && !p.signed);
  const claim = screen === "claim";
  useEffect(() => {
    document.getElementById("ps-signing")?.scrollIntoView({ block: "center" });
  }, []);
  useAfter(3200, () => {
    if (screen === "approve") api.show("done", "current");
  });
  const main = (
    <Action
      does={
        over
          ? undefined
          : needs.length > 0
            ? `Sends the ticked profiles' settings to your runner, then opens each sign-in that's needed.`
            : "Sends the ticked profiles' settings to your runner."
      }
      main={over ? undefined : needs.length > 0 ? `Sign in ${needs.length} profile${needs.length === 1 ? "" : "s"}` : "Continue"}
      onMain={() => (needs.length > 0 ? api.show("paste", "current") : api.done("work, personal", "github"))}
    >
      <Button variant="ghost" onPress={() => api.skip("Not chosen")}>
        Skip for now
      </Button>
    </Action>
  );

  return (
    <>
      <StepHead
        title="Choose the profiles for this pool"
        lead={`A profile holds your Claude, Codex or other account. A chat on the pool can use the ones you tick. Your recent ones are ticked for you.`}
      />
      {claim && (
        <Say tone="attention" title="On this pool, a sign-in lasts as long as your runner">
          A cluster runner keeps nothing once it ends. If it's replaced, purlis asks you to sign in
          again in the same place.
        </Say>
      )}
      <ul className="ps-profiles" aria-label="Your profiles">
        {profs.map((p) => {
          const isOver = p.mib > limit && ticks[p.name];
          return (
            <li key={p.name} className="ps-profile" data-over={isOver ? "" : undefined}>
              <div className="ps-profile-top">
                <Checkbox
                  isSelected={ticks[p.name]}
                  onChange={(on) => setTicks({ ...ticks, [p.name]: on })}
                  description={`${p.harness}. ${p.used}.`}
                >
                  {p.name}
                </Checkbox>
                <span className="ps-profile-state">
                  {p.signed ? <Badge variant="ok">Signed in</Badge> : <Badge>Not signed in</Badge>}
                </span>
              </div>
              <Meter
                label="Size against this pool's limit"
                value={p.mib}
                maxValue={Math.max(limit, p.mib)}
                valueLabel={`${p.mib} of ${limit} MiB${isOver ? ", over the limit" : ""}`}
              />
              {isOver && (
                <Say
                  tone="attention"
                  title="work is too big to send"
                  actions={
                    <>
                      <Button variant="primary" onPress={() => api.show("raised", "current", "Limit raised to 256 MiB")}>
                        Raise the limit to 256 MiB
                      </Button>
                      <Button variant="ghost" onPress={() => setTicks({ ...ticks, work: false })}>
                        Leave this profile out
                      </Button>
                    </>
                  }
                >
                  It's 212 MiB, and this pool lets a profile send 128 MiB. Raising the limit only
                  changes this pool, and applies from the next send. Most of the size is
                  <code> .claude/projects</code>.
                </Say>
              )}
            </li>
          );
        })}
      </ul>

      {screen === "raised" && (
        <Say tone="news" title="This pool's limit is now 256 MiB">
          work fits. You can change it again under Options.
        </Say>
      )}

      {screen === "paste" && (
        <Card variant="ask" id="ps-signing">
          <h2 className="ps-h2">Sign in personal</h2>
          <p className="ps-p">
            Claude's page is open in your browser. Sign in there, then copy the code it shows you.
          </p>
          <div className="ps-field">
            <Held label="Paste code" placeholder="Paste the code from the page" defaultValue="" />
          </div>
          <ButtonRow size="md">
            <Button variant="primary" onPress={() => api.show("done", "current")}>
              Finish sign-in
            </Button>
            <Button variant="ghost" icon={<Out />} onPress={() => api.say("Would open Claude's page again.")}>
              Open the page again
            </Button>
          </ButtonRow>
        </Card>
      )}
      {screen === "approve" && (
        <Card variant="ask" id="ps-signing">
          <h2 className="ps-h2">Sign in oss</h2>
          <Rechecking>ChatGPT's page is open in your browser. Approve there and this finishes by itself.</Rechecking>
          <ButtonRow size="md">
            <Button variant="ghost" icon={<Out />} onPress={() => api.say("Would open ChatGPT's page again.")}>
              Open the page again
            </Button>
            <Button variant="ghost" onPress={() => api.show("list", "current")}>
              Cancel this sign-in
            </Button>
          </ButtonRow>
        </Card>
      )}
      {screen === "sign-failed" && (
        <Say
          tone="trouble"
          title="personal didn't sign in"
          actions={
            <>
              <Button variant="primary" onPress={() => api.show("paste", "current")}>
                Try again
              </Button>
              <GetHelp api={api} />
            </>
          }
        >
          Claude said the code had expired. Codes last a few minutes, so start again and paste the
          new one right away. work is still signed in.
        </Say>
      )}
      {screen === "done" && (
        <Say tone="news" title="Everything you ticked is signed in">
          Chats on this pool can use work and personal.
        </Say>
      )}

      <Options>
        <Held label="Most a profile may send, size" defaultValue="" placeholder={String(limit)} help={`MiB. Recommended: 128. Now: ${limit}.`} />
        <Held label="Most a profile may send, files" defaultValue="" placeholder="8192" help="Recommended: 8192." />
        <Select
          aria-label="Default profile for new chats on this pool"
          value="work"
          onValueChange={() => undefined}
          options={[
            { value: "work", label: "Default for new chats: work" },
            { value: "personal", label: "Default for new chats: personal" },
          ]}
        />
      </Options>
      <SeeWhat
        text={`purlis pool profiles add dev-box work\npurlis pool profiles add dev-box personal\npurlis pool sign-in dev-box personal`}
      />
      {screen !== "paste" && screen !== "approve" && screen !== "sign-failed" && main}
    </>
  );
}

/* ------------------------------ Agent identity ------------------------------ */

const IDENTITY_TERM = (
  <Terms>
    <Term word="An agent identity">
      is what a chat acts as on GitHub or GitLab. It's a separate account-like identity for the
      agent, with its own limits. Your own sign-in never reaches a chat, so the agent can never act
      as you.
    </Term>
  </Terms>
);

const TOOL_LINES = `# personas/steward/persona.toml
[[tools]]
name = "gh"
secret = "vault:github-app/acme"
mint = "github-app"`;

function Identity({ api, screen }: { api: Api; screen: string }) {
  const [personas, setPersonas] = useState<Record<string, boolean>>({ steward: true, reviewer: false, writer: false });
  const lead = "So that gh and git push work in a chat on this pool. You do this once, then it stays out of your way.";
  const skip = <SkipFor api={api} line="Skipped" />;
  useAfter(3200, () => {
    if (screen === "browser") api.show("finish", "current");
  });

  if (screen === "browser")
    return (
      <>
        <StepHead title="Finish on GitHub" lead="GitHub's page is open in your browser. You're making a GitHub App for acme, then choosing its repos." />
        <Checks label="On GitHub's page">
          <CheckLine verdict="running" title="Create the App">Press Create GitHub App on the page.</CheckLine>
          <CheckLine verdict="todo" title="Install it on your repos">Choose all repos, or only some.</CheckLine>
        </Checks>
        <Rechecking>Waiting for GitHub. This moves on by itself.</Rechecking>
        <Action>
          <Button icon={<Out />} onPress={() => api.say("Would open GitHub's page again.")}>
            Open the page again
          </Button>
          <Button variant="ghost" onPress={() => api.show("github", "current")}>
            Start over
          </Button>
        </Action>
      </>
    );

  if (screen === "finish")
    return (
      <>
        <StepHead title="One last look" lead="GitHub made the App. Its key went straight into your vault, and nothing shows it." />
        <Facts aria-label="What was made">
          <Fact label="Acts as">acme-purlis-agent</Fact>
          <Fact label="Repos">12 repos in acme</Fact>
          <Fact label="May change">Code, pull requests and issues</Fact>
        </Facts>
        <Code label="The lines Finish adds to your plane" text={TOOL_LINES} />
        <Action
          does="Adds these lines for steward, gets a real short-lived token and runs read-only checks. This is the only approval it asks for."
          main="Finish"
          onMain={() => api.show("done", "current", "acme-purlis-agent on 12 repos")}
        >
          <Button variant="ghost" onPress={() => api.show("github", "current")}>
            Back
          </Button>
        </Action>
      </>
    );

  if (screen === "done" || screen === "already") {
    const already = screen === "already";
    return (
      <>
        <StepHead
          title={already ? "Already set up" : "The agent has its identity"}
          tag={<Badge variant="ok">Done</Badge>}
          lead={already ? "Acting as acme-purlis-agent on 12 repos. There's nothing to repeat." : "Acting as acme-purlis-agent on 12 repos. Chats won't ask about this again."}
        />
        <Checks label="What was proved">
          <CheckLine verdict="ok" title="Got a real token, which lasts one hour" />
          <CheckLine verdict="ok" title="Read acme/web as acme-purlis-agent" />
        </Checks>
        <p className="ps-note">To inspect or remove it, open Agent identity from the palette.</p>
        <Options>
          <h3 className="ps-h3">Who uses it</h3>
          <div className="ps-stack-sm">
            {Object.keys(personas).map((name) => (
              <Checkbox key={name} isSelected={personas[name]} onChange={(on) => setPersonas({ ...personas, [name]: on })}>
                {name}
              </Checkbox>
            ))}
          </div>
          <p className="ps-note">Ticking another persona needs no second App.</p>
        </Options>
        <Action
          main="Continue to Try it"
          onMain={() =>
            api.sim.kind === "docker" || api.sim.kind === "join"
              ? api.done("acme-purlis-agent on 12 repos", "ready")
              : api.show("mint", "current")
          }
        >
          <Button variant="ghost" onPress={() => api.say("Would open the Agent identity tab.")}>
            Open Agent identity
          </Button>
        </Action>
      </>
    );
  }

  if (screen === "gitlab" || screen === "gitlab-refused") {
    const refused = screen === "gitlab-refused";
    return (
      <>
        <StepHead title="Give the agent a GitLab token" lead={lead} />
        {IDENTITY_TERM}
        <ol className="ps-numbered">
          <li>
            Open GitLab's page for <strong>acme/web</strong>. purlis fills in what's needed: Developer
            role, <code>api</code> scope, 90 days.
          </li>
          <li>Create the token on that page and copy it.</li>
          <li>Paste it here. It goes straight into your vault.</li>
        </ol>
        <ButtonRow size="md">
          <Button icon={<Out />} onPress={() => api.say("Would open GitLab's new token page for acme/web.")}>
            Open GitLab's token page
          </Button>
        </ButtonRow>
        <div className="ps-field">
          <TextField
            type="password"
            label="Project or group token"
            help="Held in your vault. Never shown again."
            error={refused ? "This is a personal token, which acts as you. Make a project or group token instead." : undefined}
          />
        </div>
        <Say tone="attention" title="Two things about GitLab">
          GitLab can't make tokens that last under a day, so each run with this token asks you first.
          purlis warns you 14 days before it ends, and helps you replace it.
        </Say>
        <Action
          does="Stores the token in your vault, adds the tool line for steward and runs a read-only check."
          main="Check and finish"
          onMain={() => api.show(refused ? "done" : "gitlab-refused", refused ? "current" : "attention")}
        >
          {skip}
        </Action>
      </>
    );
  }

  if (screen === "other-host")
    return (
      <>
        <StepHead title="purlis can't set up git.acme.internal by itself yet" lead="One-click setup works for github.com and gitlab.com. You can still do this by hand, and it works just as well." />
        <ol className="ps-numbered">
          <li>Make a token on git.acme.internal for the agent, not for you.</li>
          <li>
            Store it: <code>purlis secret set git-acme</code>
          </li>
          <li>Add these lines to your persona. purlis notices and marks this step done.</li>
        </ol>
        <Code label="The tool line" text={`[[tools]]\nname = "gh"\nsecret = "vault:git-acme"\nenv = { GH_HOST = "git.acme.internal" }`} />
        <Action does="Moves on without an identity. Your last screen says so." main="Skip for now" onMain={() => api.skip("git.acme.internal, by hand")}>
          <Button variant="ghost" onPress={() => api.show("waiting", "waiting", "Checking for your tool line")}>
            I've added it
          </Button>
        </Action>
      </>
    );

  if (screen === "no-rights")
    return (
      <>
        <StepHead title="You'll need an admin of acme" lead="Making an App for an organisation takes an owner. Your account isn't one, and that's normal." />
        <Code label="A link for your admin" text="https://github.com/organizations/acme/settings/apps/new?state=8c1e…" copy="Copy link" />
        <p className="ps-note">It opens GitHub's page with the App filled in. It holds no secrets. Chats keep working while you wait.</p>
        <Action
          does="Marks this step as waiting and checks by itself."
          main="I've sent it"
          onMain={() => api.show("waiting", "waiting", "Waiting for your admin")}
        >
          {skip}
        </Action>
      </>
    );

  if (screen === "waiting")
    return (
      <>
        <StepHead title="Waiting for your admin" tag={<Badge variant="waiting">Waiting</Badge>} lead="You can carry on. Your pool works without this, and your last screen says what's left." />
        <Card>
          <Rechecking>Checking GitHub by itself. You'll get one entry in Needs you when it's there.</Rechecking>
        </Card>
        <Action main="Carry on to Try it" onMain={() => api.skip("Waiting for your admin")} />
      </>
    );

  if (screen === "mint" || screen === "mint-admin") {
    const adminWait = screen === "mint-admin";
    return (
      <>
        <StepHead title="Let the cluster give out tokens?" lead="This decides what a chat does when your Mac is closed." />
        <RadioGroup aria-label="When your Mac is closed" defaultValue="mint">
          <Radio value="mint" description="The cluster makes each chat a one-hour token for its own repos. It can't change CI workflows.">
            Keep working when my Mac is closed <Recommended />
          </Radio>
          <Radio value="wait" description="Nothing extra on the cluster. A push waits as a question until you're back.">
            Wait for my Mac
          </Radio>
        </RadioGroup>
        <Code
          label="What your cluster admin runs"
          text={`kubectl -n purlis-system create secret generic org-app --from-file=key.pem=<the App's key>
helm upgrade purlis-cluster-operator <chart> --namespace purlis-system --reuse-values \\
  --set 'forge.keySecretNames[0]=org-app'`}
        />
        <p className="ps-note">The App's key goes only into that one Secret. It never reaches a runner or a chat.</p>
        {adminWait ? (
          <Card>
            <Rechecking>Waiting for your admin. Checking team-prod by itself every minute.</Rechecking>
          </Card>
        ) : (
          <SeeWhat text={`spec:\n  forge:\n    appKeySecretRef: { name: org-app }\n    repos: [acme/*]\n    mintInCluster: true`} label="What changes on the Pool" />
        )}
        <Action
          does={adminWait ? undefined : "Sets this on pool team-gpu. Running chats pick it up at their next run."}
          main={adminWait ? "Carry on to Try it" : "Turn it on"}
          onMain={() => api.done("acme-purlis-agent, minted in the cluster", "ready")}
        >
          {!adminWait && (
            <Button variant="secondary" onPress={() => api.show("mint-admin", "waiting", "Waiting for your admin")}>
              Send to your admin
            </Button>
          )}
        </Action>
      </>
    );
  }

  return (
    <>
      <StepHead title="Give the agent its own GitHub identity" lead={lead} />
      {IDENTITY_TERM}
      <Facts aria-label="What purlis found">
        <Fact label="Your repos are on">github.com, in acme</Fact>
        <Fact label="It may">Read and write code, pull requests and issues</Fact>
      </Facts>
      <h2 className="ps-h3" id="ps-who">Who uses it</h2>
      <div className="ps-stack-sm" role="group" aria-labelledby="ps-who">
        {Object.keys(personas).map((name) => (
          <Checkbox key={name} isSelected={personas[name]} onChange={(on) => setPersonas({ ...personas, [name]: on })}>
            {name}
          </Checkbox>
        ))}
      </div>
      <Options>
        <Checkbox description={api.sim.kind === "docker" ? "Off by default. Most agents don't need it." : "Off by default. Tokens the cluster makes never change workflow files, whatever this says."}>
          Also let it change CI workflows
        </Checkbox>
        <Select
          aria-label="Who owns the App"
          value="acme"
          onValueChange={() => undefined}
          options={[
            { value: "acme", label: "Owned by: acme (holds your repos). Recommended" },
            { value: "me", label: "Owned by: my personal account" },
          ]}
        />
      </Options>
      <SeeWhat
        text={TOOL_LINES}
        label="The lines this ends up adding"
        note="GitHub's page does the rest. If you'd rather make the App by hand, add these lines and purlis marks this step done."
      />
      <Action
        does="Opens GitHub in your browser to create an App for acme. You confirm it there and choose its repos."
        main="Create the GitHub App"
        mainIcon={<Out />}
        onMain={() => api.show("browser", "current")}
      >
        {skip}
      </Action>
    </>
  );
}

/* ------------------------------ Try it ------------------------------ */

const TRY_SEE = `# All of these are read-only or dry runs, inside a real chat on the pool.
gh auth status
gh repo view acme/web
git ls-remote origin HEAD
git push --dry-run origin HEAD:refs/heads/purlis-setup-check`;

function TryIt({ api, screen }: { api: Api; screen: string }) {
  const pool = api.sim.kind === "docker" ? "dev-box" : "team-gpu";
  const [n, setN] = useState(0);
  useEffect(() => {
    if (screen !== "running") return;
    const tick = setInterval(() => setN((was) => was + 1), 900);
    return () => clearInterval(tick);
  }, [screen]);
  const left = api.sim.states.identity === "skipped";
  useEffect(() => {
    if (screen === "running" && n >= 4)
      if (left) api.show("except", "current", "Works, except for agent identity");
      else api.show("pass", "done", "All 4 checks passed");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [n, screen]);

  const v = (i: number) => (n > i ? "ok" : n === i ? "running" : "todo");

  if (screen === "ready")
    return (
      <>
        <StepHead title="Try it for real" lead={`Setup ends when a real chat has started on ${pool}, so you know it works before you rely on it.`} />
        <Checks label="What gets checked">
          <CheckLine verdict="todo" title={`A chat starts on ${pool}`} />
          <CheckLine verdict="todo" title="The profile is signed in" />
          <CheckLine verdict={left ? "skip" : "todo"} title={left ? "gh: skipped, there's no agent identity" : "gh works as the agent"} />
          <CheckLine verdict={left ? "skip" : "todo"} title={left ? "git push: skipped, there's no agent identity" : "git push would work (dry run)"} />
        </Checks>
        <SeeWhat text={TRY_SEE} note="This is what runs in the chat. It changes nothing on your forge." />
        <Action
          does={`Starts one chat on ${pool} and runs the checks in it. These only read, or do a dry run, and nothing is pushed.`}
          main="Start a test chat"
          onMain={() => api.show("running", "current")}
        />
      </>
    );

  if (screen === "running")
    return (
      <>
        <StepHead title="Trying it for real" lead={`purlis is starting a real chat on ${pool}. These checks read, or do a dry run. Nothing is pushed.`} />
        <Checks label="The checks">
          <CheckLine verdict={v(0)} title={`A chat starts on ${pool}`} />
          <CheckLine verdict={v(1)} title="The profile is signed in" />
          <CheckLine verdict={left ? "skip" : v(2)} title={left ? "gh: skipped, there's no agent identity" : "gh works as the agent"} />
          <CheckLine verdict={left ? "skip" : v(3)} title={left ? "git push: skipped, there's no agent identity" : "git push would work (dry run)"} />
        </Checks>
        <SeeWhat text={TRY_SEE} note="This is what runs in the chat. It changes nothing on your forge." />
      </>
    );

  if (screen === "chat-failed")
    return (
      <>
        <StepHead title="The chat didn't start" lead="Your earlier steps are fine and stay done." />
        <Checks label="The checks">
          <CheckLine verdict="missing" title={`A chat starts on ${pool}`} />
          <CheckLine verdict="skip" title="The other checks need a chat" />
        </Checks>
        <Say tone="trouble" title="The profile isn't signed in on this runner" actions={
          <>
            <Button variant="primary" onPress={() => api.go("signins", "list")}>Go to Sign-ins</Button>
            <GetHelp api={api} />
          </>
        }>
          work signed in on the old runner, and that sign-in hasn't reached the new one. Sign in once more.
        </Say>
      </>
    );

  if (screen === "push-failed")
    return (
      <>
        <StepHead title="One check didn't pass" lead="Your chat works. Only pushing to this repo is blocked, and everything else is saved." />
        <Checks label="The checks">
          <CheckLine verdict="ok" title={`A chat started on ${pool}`} />
          <CheckLine verdict="ok" title="The profile is signed in" />
          <CheckLine verdict="ok" title="gh works as acme-purlis-agent" />
          <CheckLine verdict="missing" title="git push would work (dry run)" />
        </Checks>
        <Say tone="trouble" title="acme-purlis-agent can't push to acme/web" actions={
          <>
            <Button variant="primary" icon={<Out />} onPress={() => api.show("running", "current")}>
              Add acme/web on GitHub
            </Button>
            <Button onPress={() => api.show("except", "current")}>Finish without it</Button>
            <GetHelp api={api} />
          </>
        }>
          The App isn't installed on that repo. Add it on GitHub's page, and this check runs again by itself.
        </Say>
        <SeeWhat text={`git push --dry-run origin HEAD:refs/heads/purlis-setup-check\nremote: Permission to acme/web.git denied to acme-purlis-agent[bot].`} label="The command and its answer" note="This was a dry run. Nothing was pushed." />
      </>
    );

  if (screen === "except") {
    const skippedIdentity = api.sim.states.identity === "skipped";
    return (
      <>
        <StepHead title={`${pool} works, except for this`} lead="You can start chats there now. Here's what they can and can't do." />
        <Checks label="What was checked">
          <CheckLine verdict="ok" title={`A chat starts on ${pool}`} />
          <CheckLine verdict="ok" title="work and personal are signed in" />
          <CheckLine verdict={skippedIdentity ? "skip" : "ok"} title={skippedIdentity ? "gh wasn't checked: agent identity was skipped" : "gh works as acme-purlis-agent"} />
          <CheckLine verdict={skippedIdentity ? "skip" : "missing"} title={skippedIdentity ? "git push wasn't checked" : "git push can't reach acme/web"}>
            {skippedIdentity
              ? "In these chats, gh and git push say they have no identity, and you get one Needs you entry."
              : "Other repos work. The App isn't installed on this one."}
          </CheckLine>
        </Checks>
        <div className="ps-end-actions">
          <Button size="lg" variant="primary" icon={<MessageSquarePlus aria-hidden size={16} />} onPress={() => api.say(`Would open a new chat on ${pool}.`)}>
            Open a chat on {pool}
          </Button>
          <Button size="lg" onPress={() => api.go("identity", "github")}>
            Finish {skippedIdentity ? "agent identity" : "the repo"}
          </Button>
        </div>
        <p className="ps-note">The chat picker marks this pool "Setup not finished" until then, with a link back here.</p>
      </>
    );
  }

  return (
    <>
      <StepHead title={`${pool} is ready`} tag={<Badge variant="ok">Everything passed</Badge>} lead="A real chat started there, and it can reach your repos as the agent." />
      <Checks label="What was checked">
        <CheckLine verdict="ok" title={`A chat started on ${pool}`} trail="In 11 seconds" />
        <CheckLine verdict="ok" title="work is signed in" />
        <CheckLine verdict="ok" title="gh works as acme-purlis-agent" />
        <CheckLine verdict="ok" title="git push would work on acme/web" trail="Dry run" />
      </Checks>
      <div className="ps-end-actions">
        <Button size="lg" variant="primary" icon={<MessageSquarePlus aria-hidden size={16} />} onPress={() => api.say(`Would open a new chat on ${pool}.`)}>
          Open a chat on {pool}
        </Button>
      </div>
      <p className="ps-note">Change anything later with Open setup on this pool's page.</p>
    </>
  );
}

/* ------------------------------ Around the tab ------------------------------ */

function Host({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="ps-around" aria-label={title}>
      <div className="ps-around-inner">{children}</div>
    </section>
  );
}

export function Around({ id, api }: { id: string; api: Api }) {
  const open = (step: StepId, screen?: string) => {
    api.around(undefined);
    api.go(step, screen);
  };
  if (id === "welcome")
    return (
      <Host title="Welcome">
        <h1 className="ps-title">Welcome to purlis</h1>
        <p className="ps-lead">Start a chat in a project. That's all you need today.</p>
        <ButtonRow size="lg">
          <Button variant="primary">Open a project</Button>
          <Button>Clone a repo</Button>
        </ButtonRow>
        <div className="ps-quiet-entry">
          <Button variant="link" onPress={() => open("where", "checking")}>
            Run chats on another machine
          </Button>
        </div>
      </Host>
    );
  if (id === "move")
    return (
      <Host title="Move to runner">
        <Say
          tone="attention"
          title="You don't have a pool to move this chat to"
          actions={
            <>
              <Button variant="primary" onPress={() => open("where", "checking")}>
                Set up a pool
              </Button>
              <Button variant="ghost">Keep it on this Mac</Button>
            </>
          }
        >
          A pool gives a chat its own separate machine. Setup takes about 10 minutes. This chat stays
          here and keeps working.
        </Say>
      </Host>
    );
  if (id === "missing")
    return (
      <Host title="Start a chat">
        <Say
          tone="attention"
          title="This project asks for pool team-gpu, which you don't have"
          actions={
            <>
              <Button variant="primary" onPress={() => open("where", "team")}>
                Set up team-gpu
              </Button>
              <Button>Start on this Mac</Button>
            </>
          }
        >
          Your team uses it for this project. Joining takes a couple of minutes.
        </Say>
      </Host>
    );
  if (id === "palette")
    return (
      <Host title="The palette">
        <Card>
          <div className="ps-palette-q">pool setup</div>
          <ul className="ps-palette">
            <li>
              <Button variant="ghost" icon={<Server aria-hidden size={14} />} onPress={() => open("where", "checking")}>
                Pool setup: a new pool
              </Button>
            </li>
            <li>
              <Button variant="ghost" icon={<Box aria-hidden size={14} />} onPress={() => open("signins", "list")}>
                Pool setup: dev-box, setup not finished
              </Button>
            </li>
            <li>
              <Button variant="ghost" icon={<Box aria-hidden size={14} />} onPress={() => open("try", "pass")}>
                Pool setup: team-gpu
              </Button>
            </li>
          </ul>
        </Card>
      </Host>
    );
  if (id === "picker")
    return (
      <Host title="Start a chat">
        <h1 className="ps-h2">Start a chat: where it runs</h1>
        <RadioGroup aria-label="Where it runs" defaultValue="here">
          <Radio value="here" description="This Mac">Here</Radio>
          <Radio value="dev-box" description="Docker. It can start chats, but gh and git push aren't set up.">
            dev-box <Badge variant="waiting">Setup not finished</Badge>
          </Radio>
          <Radio value="team" description="Cluster team-prod">team-gpu</Radio>
        </RadioGroup>
        <Button variant="link" onPress={() => open("identity", "github")}>
          Finish setting up dev-box
        </Button>
      </Host>
    );
  if (id === "page")
    return (
      <Host title="Pool dev-box">
        <div className="ps-title-row">
          <h1 className="ps-title">dev-box</h1>
          <Badge variant="waiting">Setup not finished</Badge>
        </div>
        <Facts aria-label="About dev-box">
          <Fact label="Kind">Docker on this Mac</Fact>
          <Fact label="Runner">Running, 2 chats on it</Fact>
          <Fact label="What's left">Agent identity was skipped</Fact>
        </Facts>
        <ButtonRow size="md">
          <Button variant="primary" onPress={() => open("identity", "github")}>
            Open setup
          </Button>
          <Button onPress={() => api.around("remove")}>Remove pool…</Button>
        </ButtonRow>
        <p className="ps-note">A pool you already had is never redone or moved. It only shows what's unfinished here, and nowhere else.</p>
      </Host>
    );
  if (id === "needs")
    return (
      <Host title="Needs you">
        <h1 className="ps-h2">Needs you</h1>
        <Say
          tone="attention"
          title="team-prod is ready"
          actions={
            <Button variant="primary" onPress={() => open("doctor", "admin-ready")}>
              Open Pool setup
            </Button>
          }
        >
          Your admin finished the install. One entry, and no more checking.
        </Say>
      </Host>
    );
  if (id === "remove")
    return (
      <Host title="Remove pool">
        <Card>
          <h1 className="ps-h2">Remove pool dev-box?</h1>
          <Checks label="What goes and what stays">
            <CheckLine verdict="missing" title="Goes: its runner and 2 chats on it">
              Those chats end. Their conversations stay in your history.
            </CheckLine>
            <CheckLine verdict="missing" title="Goes: sign-ins on this pool" />
            <CheckLine verdict="ok" title="Stays: your profiles on this Mac" />
            <CheckLine verdict="ok" title="Stays: the agent's identity, for other pools" />
          </Checks>
          <ButtonRow size="md">
            <Button variant="danger" onPress={() => api.say("Would remove the pool.")}>
              Remove pool and end 2 chats
            </Button>
            <Button onPress={() => api.around("page")}>Keep it</Button>
          </ButtonRow>
        </Card>
      </Host>
    );
  if (id === "uninstall")
    return (
      <Host title="Uninstall from a cluster">
        <h1 className="ps-h2">Uninstalling from team-prod</h1>
        <Say tone="attention" title="This affects 8 people and 14 claims">
          Their chats on team-prod would end. purlis has no button for this, so it's never done by
          accident. These are the exact commands.
        </Say>
        <Code
          label="Run these yourself"
          text={`# Removes the operator. Pools and Claims stay in place.
helm uninstall purlis-cluster-operator --namespace purlis-system

# Only once every Claim is gone:
kubectl delete -f operator/deploy/crds.yaml
kubectl delete namespace purlis-system`}
        />
      </Host>
    );
  return (
    <Host title="purlis pool">
      <h1 className="ps-h2">The same words on the command line</h1>
      <p className="ps-note">No interactive wizard. <code>purlis pool</code> runs the same checks, and this tab picks up what it did.</p>
      <Code
        label="purlis pool setup dev-box --status"
        text={`Pool dev-box (Docker on this Mac)
  done       Where chats run: Docker on this Mac
  done       Pool: dev-box
  done       Sign-ins: work, personal
  skipped    Agent identity
  done       Try it: works, except for this
Next: purlis identity add github --org acme`}
      />
    </Host>
  );
}

export function Step({ api }: { api: Api }) {
  const { at, screens, kind } = api.sim;
  const fallback: Record<StepId, string> = {
    where: "docker-ok",
    doctor: "ok",
    install: "i0",
    pool: kind === "docker" ? "form" : kind === "join" ? "team-list" : "cluster-form",
    signins: "list",
    identity: "github",
    try: "ready",
  };
  const screen = screens[at] ?? fallback[at];
  const key = `${at}:${screen}`;
  switch (at) {
    case "where":
      return <Where key={key} api={api} screen={screen} />;
    case "pool":
      return kind === "docker" ? (
        <PoolStep key={key} api={api} screen={screen} />
      ) : (
        <ClusterPool key={key} api={api} screen={screen} />
      );
    case "doctor":
      return <Doctor key={key} api={api} screen={screen} />;
    case "install":
      if (kind === "join") return <Doctor key={key} api={api} screen="found" />;
      return <Install key={key} api={api} screen={screen} />;
    case "signins":
      return <SignIns key={key} api={api} screen={screen} />;
    case "identity":
      return <Identity key={key} api={api} screen={screen} />;
    default:
      return <TryIt key={key} api={api} screen={screen} />;
  }
}

export { STEP_NAME, stepOf };
