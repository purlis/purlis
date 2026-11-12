// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch `prototype/pool-setup-remote-docker`, #6265, ADR 0123 §6). Never
 * merges to main. Stand-in data: nothing here calls Docker, ssh or a forge.
 */

/** The four routes of ADR 0123 §2, SSH first and preselected. */
/** An SSH address, built here so no literal in the source reads as an email address. */
export const AT = (user: string, host: string) => [user, host].join("\u0040");

export type Route = "ssh" | "tls" | "context" | "podman";

/** One machine purlis found in the person's own settings, for one route. */
export type Found = {
  id: string;
  /** What the person calls it: the SSH alias, the context's or the connection's name. */
  name: string;
  /** The address it resolves to, in Docker's spelling. */
  address: string;
  /** The machine's own name, as the words say it. */
  host: string;
  user: string;
  port: number;
  /** Where purlis read it, said under its name. */
  from: string;
  /** A context or connection that is no route of this kind, and why. */
  not?: { why: string; kind: "local" | "plain-tcp" };
  /** The route a context resolves to. */
  becomes?: "ssh" | "tls";
};

export const SSH_HOSTS: readonly Found[] = [
  {
    id: "build-box",
    name: "build-box",
    address: `ssh://${AT("sam", "build-box.lan")}`,
    host: "build-box.lan",
    user: "sam",
    port: 22,
    from: `${AT("sam", "build-box.lan")}, from your SSH settings`,
  },
  {
    id: "gpu-server",
    name: "gpu-server",
    address: "ssh://ci@10.0.4.12:2222",
    host: "10.0.4.12",
    user: "ci",
    port: 2222,
    from: "ci@10.0.4.12, port 2222, through jump.example.org, from your SSH settings",
  },
  {
    id: "mac-mini",
    name: "mac-mini",
    address: `ssh://${AT("sam", "mac-mini.local")}`,
    host: "mac-mini.local",
    user: "sam",
    port: 22,
    from: `${AT("sam", "mac-mini.local")}, from your SSH settings`,
  },
];

export const CONTEXTS: readonly Found[] = [
  {
    id: "build-box-ssh",
    name: "build-box-ssh",
    address: `ssh://${AT("sam", "build-box.lan")}`,
    host: "build-box.lan",
    user: "sam",
    port: 22,
    from: `Over SSH to ${AT("sam", "build-box.lan")}`,
    becomes: "ssh",
  },
  {
    id: "lab-tls",
    name: "lab-tls",
    address: "tcp://lab.example.internal:2376",
    host: "lab.example.internal",
    user: "sam",
    port: 2376,
    from: "Docker's API over TLS at lab.example.internal, with its certificates",
    becomes: "tls",
  },
  {
    id: "old-lab",
    name: "old-lab",
    address: "tcp://lab.example.internal:2375",
    host: "lab.example.internal",
    user: "sam",
    port: 2375,
    from: "Docker's API at lab.example.internal, without TLS",
    not: { kind: "plain-tcp", why: "Without TLS, so purlis won't use it." },
  },
  {
    id: "desktop-linux",
    name: "desktop-linux",
    address: "unix:///Users/sam/.docker/run/docker.sock",
    host: "this machine",
    user: "sam",
    port: 0,
    from: "Docker Desktop on this machine",
    not: { kind: "local", why: "This is Docker on this machine. Choose This machine under Where." },
  },
];

export const PODMAN_CONNECTIONS: readonly Found[] = [
  {
    id: "nas",
    name: "nas",
    address: `ssh://${AT("sam", "nas.lan")}:22/run/user/1000/podman/podman.sock`,
    host: "nas.lan",
    user: "sam",
    port: 22,
    from: "Podman as sam on nas.lan, with the key ~/.ssh/id_ed25519",
  },
];

export const FOUND: Record<Route, readonly Found[]> = {
  ssh: SSH_HOSTS,
  tls: [],
  context: CONTEXTS,
  podman: PODMAN_CONNECTIONS,
};

/** The key a machine presents, and the one the person's own ssh already knows for it. */
export const HOST_KEY = {
  type: "ED25519",
  fingerprint: "SHA256:q3Xv8mJb0dHk2pT9wRzL4nC7aYs1eF6uGi5oQ0lN2Vc",
  /** What it presents after the machine was reinstalled. */
  changed: "SHA256:Zt7bE2kPq9cW4nYx1hLm6sR0aVf3dJu8gO5iT2eB7Qk",
} as const;

/** The CA a TLS engine's certificate is checked against. */
export const TLS_CA = {
  subject: "CN=lab.example.internal CA, O=Example Lab",
  fingerprint:
    "7A:3F:91:0C:5E:D2:84:B6:19:EE:47:2A:C0:6D:F3:58:91:0B:7C:AA:E4:32:6F:1D:88:C5:09:B7:2E:64:D1:3F",
  changed:
    "C1:08:5B:7E:22:A9:F0:43:6D:91:BC:0E:57:34:A8:F2:6C:1D:E9:80:3B:47:A5:12:D6:9F:0C:68:B3:E1:24:7A",
  files: {
    ca: "~/.docker/lab/ca.pem",
    cert: "~/.docker/lab/cert.pem",
    key: "~/.docker/lab/key.pem",
  },
} as const;

export const ENGINE_ID = {
  id: "6XKQ:3ZBW:JN4T:PL2R:QF7M:YH5D:CE8S:VA9G:RT1U:KW6N:MB3X:ZD4E",
  changed: "H2LP:9QXA:CR6V:TN8B:WE3K:JS1F:MD7Y:GU5Z:BO4I:PH2C:XL9R:FA6T",
} as const;

/**
 * **What the engine check can find** (ADR 0123 §7), the pinned-key, engine-ID and plain-TCP rows
 * left out: those are the pin-changed Notice and the refusal, each a scene of its own.
 */
export type FindingId =
  | "name"
  | "answers"
  | "sign-in"
  | "no-engine"
  | "not-allowed"
  | "tls-ca"
  | "tls-expired"
  | "tls-name"
  | "daemon"
  | "arch"
  | "disk"
  | "pull"
  | "context-moved";

/** The checks Connect makes, in the order it makes them, each with the finding it can give. */
export const CHECKS: readonly { title: string; finding: FindingId | FindingId[] }[] = [
  { title: "purlis can find the machine", finding: "name" },
  { title: "The machine answers", finding: "answers" },
  { title: "It accepts your sign-in", finding: "sign-in" },
  { title: "Docker is running there", finding: "no-engine" },
  { title: "Your account may use Docker there", finding: "not-allowed" },
  { title: "Its certificate checks out", finding: ["tls-ca", "tls-expired", "tls-name"] },
  { title: "Docker there is new enough and keeps runners apart", finding: "daemon" },
  { title: "There is a runner image for its kind of processor", finding: "arch" },
  { title: "It has room for a runner", finding: "disk" },
  { title: "It can download the runner image", finding: "pull" },
];

/** A finding's words: a title of what happened, a sentence of what to do, and its remedy. */
export type Said = {
  title: string;
  says: string;
  /** What that means, for someone who has never run Docker. */
  plainly?: string;
  /** The main remedy's words. */
  remedy: string;
  /** Commands under See what this does, where a remedy is done on the other machine. */
  commands?: string;
  /** A finding that is only about the route: TLS has no sign-in, SSH has no certificate. */
  only?: "ssh" | "tls";
};

export function said(id: FindingId, host: string, user: string, port: number): Said {
  switch (id) {
    case "name":
      return {
        title: `purlis couldn't find ${host}`,
        says: `purlis couldn't find a machine called ${host}. Check the name, or pick it from your SSH hosts.`,
        remedy: "Change the machine",
      };
    case "answers":
      return {
        title: `${host} didn't answer`,
        says: `${host} didn't answer on port ${port}. Check that it's on and on this network, and connect your VPN if you use one.`,
        remedy: "Check again",
      };
    case "sign-in":
      return {
        title: `${host} didn't accept your sign-in`,
        says: `purlis signs in with your SSH agent and never asks for a password. Add your key to the agent with ssh-add, then check again.`,
        plainly:
          "Your SSH agent is the program on this machine that holds your keys while you work.",
        remedy: "Check again",
        commands: "ssh-add ~/.ssh/id_ed25519\nssh-add -l",
        only: "ssh",
      };
    case "no-engine":
      return {
        title: `Docker isn't running on ${host}`,
        says: `${host} has no Docker running that ${user} can use. Start Docker there, or name its socket under Options.`,
        plainly:
          "A socket is the file Docker listens on. Most machines use the usual one, so purlis tries that first.",
        remedy: "Check again",
        commands: `ssh ${user}@${host} 'sudo systemctl start docker'`,
      };
    case "not-allowed":
      return {
        title: `${user} can't use Docker on ${host}`,
        says: `${user} on ${host} isn't allowed to use Docker. On ${host}, add ${user} to the docker group, or use rootless Docker or Podman.`,
        plainly: `Anyone who may use Docker there can do anything on ${host}, so it's off for new accounts.`,
        remedy: "Check again",
        commands: `sudo usermod -aG docker ${user}\n# then sign out of ${host} and back in`,
      };
    case "tls-ca":
      return {
        title: `${host}'s certificate doesn't match`,
        says: `${host}'s certificate isn't signed by the CA you chose. Check that ${TLS_CA.files.ca} is the one that came with this machine's certificates.`,
        plainly: "The CA file is how purlis knows it's talking to the right machine.",
        remedy: "Choose other files",
        only: "tls",
      };
    case "tls-expired":
      return {
        title: `${host}'s certificate has expired`,
        says: `${host}'s certificate has expired. Make it a new one, then check again. The file to renew is the server certificate on ${host}.`,
        remedy: "Check again",
        only: "tls",
      };
    case "tls-name":
      return {
        title: `${host}'s certificate is for another name`,
        says: `${host}'s certificate is for another name (docker.lab.example.internal). Connect with that name, or make it a certificate for ${host}.`,
        remedy: "Use docker.lab.example.internal",
        only: "tls",
      };
    case "daemon":
      return {
        title: `Docker on ${host} can't keep runners apart`,
        says: `Docker on ${host} doesn't give containers its built-in seccomp profile, so purlis makes no runner there. Set its seccomp profile back to its default, then check again.`,
        plainly:
          "That profile is what stops a chat in one runner from reaching the machine under it.",
        remedy: "Check again",
        commands: `ssh ${user}@${host} 'docker info --format "{{json .SecurityOptions}}"'`,
      };
    case "arch":
      return {
        title: `There's no runner image for ${host}`,
        says: `${host} is linux/riscv64, and the runner image is built for linux/amd64 and linux/arm64. Pick another machine.`,
        remedy: "Change the machine",
      };
    case "disk":
      return {
        title: `${host} has no room for a runner`,
        says: `Docker's disk on ${host} is full: it has 1.2 GB free, and a runner needs at least 8 GB. Free some space there, then check again.`,
        remedy: "Check again",
        commands: `ssh ${user}@${host} 'docker system df'\nssh ${user}@${host} 'docker image prune'`,
      };
    case "pull":
      return {
        title: `${host} couldn't download the runner image`,
        says: `${host} couldn't download the runner image. Check that ${host} can reach ghcr.io.`,
        remedy: "Check again",
        commands: `ssh ${user}@${host} 'docker pull ghcr.io/purlis/runner@sha256:4be1…'`,
      };
    case "context-moved":
      return {
        title: "Your context points somewhere else now",
        says: "Your context build-box-ssh now points somewhere else than this pool. Connect again to follow it, or keep the pool as it is.",
        remedy: "Connect again",
      };
  }
}

/** Every finding, in §7's order, for the gallery and the prototype's own control. */
export const FINDINGS: readonly FindingId[] = [
  "name",
  "answers",
  "sign-in",
  "no-engine",
  "not-allowed",
  "tls-ca",
  "tls-expired",
  "tls-name",
  "daemon",
  "arch",
  "disk",
  "pull",
];
