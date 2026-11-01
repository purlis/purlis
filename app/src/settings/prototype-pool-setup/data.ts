// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/pool-setup, spec #4469). Stand-in data only: nothing here
 * calls Docker, a cluster or a forge.
 */

export type Kind = "docker" | "cluster" | "join";
export type StepId = "where" | "doctor" | "install" | "pool" | "signins" | "identity" | "try";

/** Q29's five, and `later`: a step not reached yet (D-proto-1 in the README). */
export type StepState = "done" | "current" | "attention" | "skipped" | "waiting" | "later";

export const STEPS: Record<Kind, StepId[]> = {
  docker: ["where", "pool", "signins", "identity", "try"],
  cluster: ["where", "doctor", "install", "pool", "signins", "identity", "try"],
  join: ["where", "doctor", "install", "pool", "signins", "identity", "try"],
};

export const STEP_NAME: Record<StepId, string> = {
  where: "Where chats run",
  doctor: "Check the cluster",
  install: "Install",
  pool: "Pool",
  signins: "Sign-ins",
  identity: "Agent identity",
  try: "Try it",
};

export const STATE_WORD: Record<StepState, string> = {
  done: "Done",
  current: "You are here",
  attention: "Needs attention",
  skipped: "Skipped",
  waiting: "Waiting on someone",
  later: "",
};

/** The whole of what the tab shows. Choices are `screens`, one per step, and step states. */
export type Sim = {
  kind: Kind;
  at: StepId;
  /** Each step's own screen; a step without one shows its first. */
  screens: Partial<Record<StepId, string>>;
  /** Step states that aren't "before here, done; after here, later". */
  states: Partial<Record<StepId, StepState>>;
  /** Short line under a step's name in the list. */
  lines: Partial<Record<StepId, string>>;
  /** Screens outside the tab: Welcome, the chat picker and so on. */
  around?: string;
  /** "Welcome back" over the step. */
  resumed?: boolean;
};

export function stateOf(sim: Sim, step: StepId): StepState {
  const given = sim.states[step];
  if (given !== undefined && !(step === sim.at && given === "done")) return given;
  if (step === sim.at) return given ?? "current";
  const order = STEPS[sim.kind];
  return order.indexOf(step) < order.indexOf(sim.at) ? "done" : "later";
}

type Seed = Partial<Omit<Sim, "kind" | "at">> & { kind: Kind; at: StepId };

/** One entry of the state index. */
export type Scenario = {
  id: string;
  group: string;
  title: string;
  /** Which spec stories or rulings it answers. */
  shows: string;
  sim: Seed;
};

const DOCKER_DONE_LINES = {
  where: "Docker on this Mac",
  pool: "dev-box",
  signins: "work, personal",
  identity: "acme-purlis-agent on 12 repos",
};
const CLUSTER_LINES = {
  where: "Cluster team-prod",
  doctor: "Nothing missing",
  install: "Operator 0.9.2 running",
  pool: "team-gpu",
  signins: "work",
  identity: "acme-purlis-agent, minted in the cluster",
};

const d = (at: StepId, screen?: string, more: Partial<Seed> = {}): Seed => ({
  kind: "docker",
  at,
  screens: screen ? { [at]: screen } : {},
  ...more,
});
const c = (at: StepId, screen?: string, more: Partial<Seed> = {}): Seed => ({
  kind: "cluster",
  at,
  screens: screen ? { [at]: screen } : {},
  ...more,
});
const j = (at: StepId, screen?: string, more: Partial<Seed> = {}): Seed => ({
  kind: "join",
  at,
  screens: screen ? { [at]: screen } : {},
  ...more,
});
const around = (id: string): Seed => ({ kind: "docker", at: "where", around: id });

const G = {
  start: "1 · Where chats run",
  docker: "2 · Docker: Pool",
  sign: "3 · Sign-ins",
  id: "4 · Agent identity",
  doctor: "5 · Cluster: Check",
  install: "6 · Cluster: Install",
  cpool: "7 · Cluster: Pool and minting",
  join: "8 · Joining a team's pool",
  admin: "9 · Send to your admin",
  end: "10 · Try it and the last screen",
  around: "11 · Around the tab",
};

export const SCENARIOS: Scenario[] = [
  { id: "where-checking", group: G.start, title: "Looking at what you have", shows: "Story 3", sim: d("where", "checking") },
  { id: "where-docker", group: G.start, title: "Docker is ready, recommended", shows: "Stories 2, 3; Q27", sim: d("where", "docker-ok") },
  { id: "where-missing", group: G.start, title: "Docker missing, rechecking", shows: "Story 5; Q28", sim: d("where", "docker-missing", { states: { where: "attention" } }) },
  { id: "where-stopped", group: G.start, title: "Docker installed, not running", shows: "Story 5; Q28", sim: d("where", "docker-stopped", { states: { where: "attention" } }) },
  { id: "where-found", group: G.start, title: "Docker found on recheck", shows: "Story 5", sim: d("where", "docker-found") },
  { id: "where-no-cluster", group: G.start, title: "Chose a cluster, there is none", shows: "Story 6", sim: { kind: "cluster", at: "where", screens: { where: "no-cluster" }, states: { where: "attention" } } },
  { id: "where-cluster", group: G.start, title: "A cluster is available", shows: "Story 4", sim: { kind: "cluster", at: "where", screens: { where: "cluster-ok" } } },
  { id: "where-team", group: G.start, title: "Your team has pools here", shows: "Q37", sim: { kind: "join", at: "where", screens: { where: "team" } } },
  { id: "resume", group: G.start, title: "Coming back: resumed where you stopped", shows: "Story 18", sim: d("signins", "list", { resumed: true, lines: { where: "Docker on this Mac", pool: "dev-box" } }) },

  { id: "pool-form", group: G.docker, title: "Name it, defaults chosen", shows: "Stories 14, 25, 26", sim: d("pool", "form", { lines: { where: "Docker on this Mac" } }) },
  { id: "pool-options", group: G.docker, title: "Options and See what this does open", shows: "Stories 15, 16, 26; Q30", sim: d("pool", "form-open", { lines: { where: "Docker on this Mac" } }) },
  { id: "pool-signin", group: G.docker, title: "The image needs a GitHub sign-in", shows: "Beyond the rulings; docs/runner-image.md", sim: d("pool", "registry", { states: { pool: "attention" }, lines: { where: "Docker on this Mac" } }) },
  { id: "pool-creating", group: G.docker, title: "Creating, with progress", shows: "Story 19", sim: d("pool", "creating", { lines: { where: "Docker on this Mac" } }) },
  { id: "pool-failed", group: G.docker, title: "A failed step", shows: "Stories 21, 22; Q33", sim: d("pool", "failed", { states: { pool: "attention" }, lines: { where: "Docker on this Mac", pool: "Docker stopped" } }) },
  { id: "pool-taken", group: G.docker, title: "That name is taken", shows: "Story 21", sim: d("pool", "taken", { states: { pool: "attention" }, lines: { where: "Docker on this Mac" } }) },
  { id: "pool-image", group: G.docker, title: "Your team's image breaks the contract", shows: "Story 27", sim: d("pool", "image-bad", { states: { pool: "attention" }, lines: { where: "Docker on this Mac" } }) },
  { id: "pool-hand", group: G.docker, title: "Noticed: you made it by hand", shows: "Stories 17, 58; Q44", sim: d("pool", "by-hand", { lines: { where: "Docker on this Mac" } }) },
  { id: "pool-reopen", group: G.docker, title: "A change that needs a new runner", shows: "Story 53; Q39", sim: d("pool", "change", { lines: DOCKER_DONE_LINES, states: { where: "done", signins: "done", identity: "done", try: "done" } }) },

  { id: "sign-list", group: G.sign, title: "Recent profiles ticked, sizes shown", shows: "Stories 28, 29; Q41", sim: d("signins", "list", { lines: { where: "Docker on this Mac", pool: "dev-box" } }) },
  { id: "sign-over", group: G.sign, title: "A profile over its limit", shows: "Story 30; Q41", sim: d("signins", "over", { states: { signins: "attention" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work is over its limit" } }) },
  { id: "sign-raised", group: G.sign, title: "Limit raised in place", shows: "Story 30", sim: d("signins", "raised", { lines: { where: "Docker on this Mac", pool: "dev-box" } }) },
  { id: "sign-browser", group: G.sign, title: "Signing in: browser, paste code", shows: "Story 31", sim: d("signins", "paste", { lines: { where: "Docker on this Mac", pool: "dev-box" } }) },
  { id: "sign-approve", group: G.sign, title: "Signing in: approve in the browser", shows: "Story 31", sim: d("signins", "approve", { lines: { where: "Docker on this Mac", pool: "dev-box" } }) },
  { id: "sign-failed", group: G.sign, title: "A sign-in failed", shows: "Story 21", sim: d("signins", "sign-failed", { states: { signins: "attention" }, lines: { where: "Docker on this Mac", pool: "dev-box" } }) },
  { id: "sign-done", group: G.sign, title: "Everything ticked is signed in", shows: "Story 31", sim: d("signins", "done", { lines: { where: "Docker on this Mac", pool: "dev-box" } }) },

  { id: "id-github", group: G.id, title: "GitHub: why, and for whom", shows: "Stories 32, 33; Q3, Q4, Q5", sim: d("identity", "github", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-browser", group: G.id, title: "Waiting on GitHub's page", shows: "Story 34", sim: d("identity", "browser", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-finish", group: G.id, title: "Finish shows the exact lines", shows: "Q11, Q17", sim: d("identity", "finish", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-done", group: G.id, title: "Proved: what it acts as", shows: "Q17", sim: d("identity", "done", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-already", group: G.id, title: "Already set up", shows: "Story 35; Q40", sim: d("identity", "already", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal", identity: "acme-purlis-agent on 12 repos" } }) },
  { id: "id-gitlab", group: G.id, title: "GitLab: a project or group token", shows: "Q12, Q19", sim: d("identity", "gitlab", { lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-gitlab-refused", group: G.id, title: "GitLab: a personal token is refused", shows: "Q12", sim: d("identity", "gitlab-refused", { states: { identity: "attention" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-other", group: G.id, title: "Another forge host", shows: "Story 36; Q49", sim: d("identity", "other-host", { states: { identity: "attention" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal", identity: "git.acme.internal" } }) },
  { id: "id-norights", group: G.id, title: "No rights on the organisation", shows: "Q8", sim: d("identity", "no-rights", { states: { identity: "attention" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal" } }) },
  { id: "id-waiting", group: G.id, title: "Waiting for the organisation's admin", shows: "Q8", sim: d("identity", "waiting", { states: { identity: "waiting" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal", identity: "Waiting for your admin" } }) },

  { id: "doc-running", group: G.doctor, title: "Doctor running", shows: "Story 38", sim: c("doctor", "running", { lines: { where: "Cluster team-prod" } }) },
  { id: "doc-ok", group: G.doctor, title: "Nothing stops the install", shows: "Story 38", sim: c("doctor", "ok", { lines: { where: "Cluster team-prod" } }) },
  { id: "doc-old", group: G.doctor, title: "The cluster lacks something", shows: "Story 38", sim: c("doctor", "old", { states: { doctor: "attention" }, lines: { where: "Cluster team-prod", doctor: "Kubernetes is too old" } }) },
  { id: "doc-unreachable", group: G.doctor, title: "Couldn't reach the cluster", shows: "Story 21", sim: c("doctor", "unreachable", { states: { doctor: "attention" }, lines: { where: "Cluster team-prod" } }) },

  { id: "ins-start", group: G.install, title: "Four buttons, none pressed", shows: "Stories 39, 41; Q36", sim: c("install", "i0", { lines: { where: "Cluster team-prod", doctor: "Nothing stopping it" } }) },
  { id: "ins-mid", group: G.install, title: "Two of four done", shows: "Story 39", sim: c("install", "i2", { lines: { where: "Cluster team-prod", doctor: "Nothing stopping it" } }) },
  { id: "ins-key", group: G.install, title: "A signing key is already there", shows: "Story 40", sim: c("install", "key-kept", { lines: { where: "Cluster team-prod", doctor: "Nothing stopping it" } }) },
  { id: "ins-failed", group: G.install, title: "The operator didn't start", shows: "Story 21", sim: c("install", "op-failed", { states: { install: "attention" }, lines: { where: "Cluster team-prod", doctor: "Nothing stopping it" } }) },
  { id: "ins-done", group: G.install, title: "Installed", shows: "Story 39", sim: c("install", "i4", { lines: { where: "Cluster team-prod", doctor: "Nothing stopping it" } }) },

  { id: "cp-form", group: G.cpool, title: "First Pool on the cluster", shows: "Story 37", sim: c("pool", "cluster-form", { lines: { where: "Cluster team-prod", doctor: "Nothing missing", install: "Operator 0.9.2 running" } }) },
  { id: "cp-claim", group: G.cpool, title: "Sign-ins on a fresh claim", shows: "Q41; #3819", sim: c("signins", "claim", { lines: { where: "Cluster team-prod", doctor: "Nothing missing", install: "Operator 0.9.2 running", pool: "team-gpu" } }) },
  { id: "cp-mint", group: G.cpool, title: "Agent identity with the minting opt-in", shows: "Stories 37; Q23, Q36", sim: c("identity", "mint", { lines: { where: "Cluster team-prod", doctor: "Nothing missing", install: "Operator 0.9.2 running", pool: "team-gpu", signins: "work" } }) },
  { id: "cp-mint-admin", group: G.cpool, title: "Minting opt-in needs the admin", shows: "Q36, #3807", sim: c("identity", "mint-admin", { states: { identity: "waiting" }, lines: { where: "Cluster team-prod", doctor: "Nothing missing", install: "Operator 0.9.2 running", pool: "team-gpu", signins: "work", identity: "Waiting for your admin" } }) },

  { id: "join-list", group: G.join, title: "Your team has these pools", shows: "Stories 42, 43; Q36, Q37", sim: j("pool", "team-list", { lines: { where: "Cluster team-prod", doctor: "Found: nothing missing", install: "Found: operator 0.9.2" } }) },
  { id: "join-joined", group: G.join, title: "Joined, on to your own sign-ins", shows: "Story 43", sim: j("pool", "joined", { lines: { where: "Cluster team-prod", doctor: "Found: nothing missing", install: "Found: operator 0.9.2", pool: "team-gpu" } }) },
  { id: "join-found", group: G.join, title: "Going back to a step that was found done", shows: "Story 42", sim: j("doctor", "found", { states: { pool: "later" }, lines: { where: "Cluster team-prod", install: "Found: operator 0.9.2" } }) },

  { id: "adm-why", group: G.admin, title: "You can't install this yourself", shows: "Story 44; Q38", sim: c("doctor", "no-rights", { states: { doctor: "attention" }, lines: { where: "Cluster team-prod", doctor: "You need an admin" } }) },
  { id: "adm-send", group: G.admin, title: "Send to your admin", shows: "Stories 44, 46; Q38, Q48", sim: c("doctor", "send", { states: { doctor: "attention" }, lines: { where: "Cluster team-prod", doctor: "You need an admin" } }) },
  { id: "adm-wait", group: G.admin, title: "Waiting for your admin", shows: "Story 45; Q38", sim: c("doctor", "admin-wait", { states: { doctor: "waiting", install: "waiting" }, lines: { where: "Cluster team-prod", doctor: "Waiting for your admin", install: "Waiting for your admin" } }) },
  { id: "adm-ready", group: G.admin, title: "Your admin finished", shows: "Story 45", sim: c("doctor", "admin-ready", { states: { install: "done" }, lines: { where: "Cluster team-prod", doctor: "Nothing missing", install: "Operator 0.9.2 running" } }) },

  { id: "try-ready", group: G.end, title: "Try it: its own button", shows: "Stories 19, 47; Q31", sim: d("try", "ready", { lines: DOCKER_DONE_LINES }) },
  { id: "try-running", group: G.end, title: "Running the checks", shows: "Stories 47, 48; Q32", sim: d("try", "running", { lines: DOCKER_DONE_LINES }) },
  { id: "try-failed", group: G.end, title: "git push check failed", shows: "Stories 21, 22", sim: d("try", "push-failed", { states: { try: "attention" }, lines: DOCKER_DONE_LINES }) },
  { id: "try-chat-failed", group: G.end, title: "The chat didn't start", shows: "Stories 21, 47", sim: d("try", "chat-failed", { states: { try: "attention" }, lines: DOCKER_DONE_LINES }) },
  { id: "end-pass", group: G.end, title: "Last screen: everything works", shows: "Story 49", sim: d("try", "pass", { states: { try: "done" }, lines: DOCKER_DONE_LINES }) },
  { id: "end-except", group: G.end, title: "Last screen: works, except for this", shows: "Story 50; Q32", sim: d("try", "except", { states: { identity: "skipped" }, lines: { where: "Docker on this Mac", pool: "dev-box", signins: "work, personal", identity: "Skipped" } }) },
  { id: "end-cluster", group: G.end, title: "Last screen on a team's pool", shows: "Stories 43, 49", sim: j("try", "pass", { states: { try: "done" }, lines: { ...CLUSTER_LINES, doctor: "Found: nothing missing", install: "Found: operator 0.9.2" } }) },
  { id: "help", group: G.end, title: "Get help opens a chat", shows: "Stories 59, 60; Q33", sim: d("pool", "help", { states: { pool: "attention" }, lines: { where: "Docker on this Mac", pool: "Docker stopped" } }) },

  { id: "ar-welcome", group: G.around, title: "Welcome: one quiet entry", shows: "Story 7; Q43", sim: around("welcome") },
  { id: "ar-move", group: G.around, title: "Moving a chat with no pool", shows: "Story 8", sim: around("move") },
  { id: "ar-missing", group: G.around, title: "Starting on a pool you don't have", shows: "Story 9", sim: around("missing") },
  { id: "ar-palette", group: G.around, title: "From the palette", shows: "Story 10", sim: around("palette") },
  { id: "ar-picker", group: G.around, title: "Chat picker: Setup not finished", shows: "Story 51; Q47", sim: around("picker") },
  { id: "ar-page", group: G.around, title: "A pool's page: Open setup", shows: "Stories 52, 54; Q39, Q42", sim: around("page") },
  { id: "ar-needs", group: G.around, title: "Needs you entry", shows: "Story 45; Q38", sim: around("needs") },
  { id: "ar-remove", group: G.around, title: "Removing a pool", shows: "Story 55; Q45", sim: around("remove") },
  { id: "ar-uninstall", group: G.around, title: "Uninstalling from a cluster", shows: "Story 56; Q45", sim: around("uninstall") },
  { id: "ar-cli", group: G.around, title: "purlis pool says the same", shows: "Story 57; Q44", sim: around("cli") },
];

export function simOf(id: string): Sim {
  const found = SCENARIOS.find((one) => one.id === id) ?? SCENARIOS[1];
  return { screens: {}, states: {}, lines: {}, ...found.sim };
}
