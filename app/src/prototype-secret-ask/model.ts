// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (spec #5054, ADR 0109, #5070): the Secret ask's states, in memory. Nothing
 * here calls the core; every name and vault is a stand-in.
 */

export type Mode = "value" | "entry";

/** Why this ask is drawn the way it is, beyond a plain missing key. */
export type Reason =
  | { kind: "missing" }
  | { kind: "replace"; said: string; linkedTo?: string; linkedFrom?: number }
  | { kind: "broken"; was: string }
  | { kind: "runner"; runner: string }
  | { kind: "no-vault" };

export type Ask = {
  chat: string;
  persona: string;
  vault: string;
  key: string;
  /** Who raised it: purlis's broker (a command missed the key) or the chat itself. */
  raisedBy: "broker" | "chat";
  /** What the broker was running when it missed the key. */
  command?: string;
  /** The chat's own words, masked and bounded: never purlis's. */
  purpose?: string;
  /** The persona's committed Secret need, when it declares this key. */
  declared?: string;
  waiting: string[];
  reason: Reason;
  age: number;
};

export type Settled = { kind: "filled"; how: "value" | "link"; vault: string; key: string; to?: string } | { kind: "declined" };

export type World = {
  ask: Ask | null;
  mode: Mode;
  changing: boolean;
  keyAs: string;
  vaultAs: string;
  entry: string;
  valueFilled: boolean;
  open: boolean;
  settled: Settled | null;
  needOffered: "open" | "proposed" | "no" | null;
  typed: string;
};

export type Scene = {
  id: string;
  name: string;
  stories: string;
  make: () => World;
  /** A button on the prototype's bar that plays an event. */
  pretend?: { label: string; run: (w: World) => void };
};

export const VAULTS = ["devops", "work", "personal"];

/** The person's entries, as the picker lists them; a link names its target. */
export const ENTRIES = [
  { vault: "work", key: "STRIPE_TEST_KEY" },
  { vault: "work", key: "STRIPE_LIVE_KEY" },
  { vault: "work", key: "GITHUB_TOKEN" },
  { vault: "personal", key: "OPENAI_API_KEY" },
  { vault: "personal", key: "STRIPE_SANDBOX" },
  { vault: "devops", key: "KUBECONFIG" },
  { vault: "devops", key: "SENTRY_DSN" },
];

const base = (ask: Partial<Ask> = {}): Ask => ({
  chat: "developer 7",
  persona: "devops",
  vault: "devops",
  key: "STRIPE_API_KEY",
  raisedBy: "chat",
  waiting: ["developer 7"],
  reason: { kind: "missing" },
  age: 0,
  ...ask,
});

const world = (ask: Ask | null, more: Partial<World> = {}): World => ({
  ask,
  mode: "value",
  changing: false,
  keyAs: ask?.key ?? "",
  vaultAs: ask?.vault ?? "",
  entry: "",
  valueFilled: false,
  open: true,
  settled: null,
  needOffered: null,
  typed: "",
  ...more,
});

const DECLARED = "Runs the checkout tests against Stripe's test mode.";
const AGENT = "I need it to run the checkout e2e suite against Stripe test mode for #5012.";

export const SCENES: Scene[] = [
  {
    id: "new-value",
    name: "New value (raised by purlis, declared)",
    stories: "1, 13, 15, 22",
    make: () =>
      world(base({ raisedBy: "broker", command: "secret exec -- pnpm e2e:checkout", declared: DECLARED })),
  },
  {
    id: "pick-entry",
    name: "Pick an existing entry → Linked entry",
    stories: "23, 24",
    make: () => world(base({ declared: DECLARED, purpose: AGENT }), { mode: "entry", entry: "work/STRIPE_TEST_KEY" }),
  },
  {
    id: "pick-entry-empty",
    name: "Pick an existing entry, nothing picked yet",
    stories: "23",
    make: () => world(base({ declared: DECLARED, purpose: AGENT }), { mode: "entry" }),
  },
  {
    id: "rename-vault",
    name: "Rename the key and choose the vault",
    stories: "25, 26",
    make: () =>
      world(base({ key: "stripe_key", purpose: "Need a stripe key to test checkout." }), {
        changing: true,
        keyAs: "STRIPE_TEST_KEY",
        vaultAs: "work",
      }),
  },
  {
    id: "declared",
    name: "Declared: persona's purpose and the chat's words",
    stories: "14, 15",
    make: () => world(base({ declared: DECLARED, purpose: AGENT })),
  },
  {
    id: "undeclared",
    name: "Not declared by this persona (quiet tag)",
    stories: "5, 14, 16",
    make: () =>
      world(
        base({
          key: "TWILIO_AUTH_TOKEN",
          purpose: "The user asked me to send a test SMS from the staging notifier; I need Twilio's auth token for that.",
        }),
      ),
  },
  {
    id: "waiting-3",
    name: "3 waiting on one key",
    stories: "20",
    make: () =>
      world(base({ declared: DECLARED, purpose: AGENT, waiting: ["developer 7", "reviewer 2", "qa 1"], age: 4 })),
  },
  {
    id: "replace",
    name: "Replace: the chat says the value doesn't work",
    stories: "11, 17",
    make: () =>
      world(
        base({
          declared: DECLARED,
          purpose: AGENT,
          reason: { kind: "replace", said: "api.stripe.com answered 401: Invalid API Key provided." },
        }),
      ),
  },
  {
    id: "replace-linked",
    name: "Replace a Linked entry (names the target)",
    stories: "17, 18",
    make: () =>
      world(
        base({
          declared: DECLARED,
          purpose: AGENT,
          reason: {
            kind: "replace",
            said: "api.stripe.com answered 401: Invalid API Key provided.",
            linkedTo: "work/STRIPE_TEST_KEY",
            linkedFrom: 3,
          },
        }),
      ),
  },
  {
    id: "broken-link",
    name: "Broken link",
    stories: "19",
    make: () =>
      world(
        base({
          raisedBy: "broker",
          command: "secret exec -- pnpm e2e:checkout",
          declared: DECLARED,
          reason: { kind: "broken", was: "old-work/STRIPE_TEST_KEY" },
        }),
        { mode: "entry" },
      ),
  },
  {
    id: "runner",
    name: "Runner can't receive",
    stories: "47, 49, 50",
    make: () =>
      world(base({ chat: "developer 9", waiting: ["developer 9"], declared: DECLARED, purpose: AGENT, reason: { kind: "runner", runner: "pool mac-ci · runner 3" } })),
  },
  {
    id: "no-vault",
    name: "No vault yet: made on the first fill",
    stories: "27",
    make: () => world(base({ persona: "payments", vault: "payments", declared: DECLARED, purpose: AGENT, reason: { kind: "no-vault" } })),
  },
  {
    id: "not-now",
    name: "Not now (declined)",
    stories: "30, 32, 33",
    make: () => world(base({ declared: DECLARED, purpose: AGENT }), { settled: { kind: "declined" } }),
  },
  {
    id: "filled-offer",
    name: "Filled, undeclared: offer the Secret need",
    stories: "31, 43",
    make: () =>
      world(base({ key: "TWILIO_AUTH_TOKEN", purpose: "Send a test SMS." }), {
        settled: { kind: "filled", how: "link", vault: "devops", key: "TWILIO_AUTH_TOKEN", to: "personal/TWILIO_TOKEN" },
        needOffered: "open",
      }),
  },
  {
    id: "arrives-while-typing",
    name: "Arrives while you type: never steals focus",
    stories: "21",
    make: () => world(null, { typed: "pnpm te" }),
    pretend: {
      label: "Ask arrives",
      run: (w) => {
        w.ask = base({ declared: DECLARED, purpose: AGENT });
        w.keyAs = w.ask.key;
        w.vaultAs = w.ask.vault;
        w.open = false;
      },
    },
  },
];

export const needsCount = (w: World) => (w.ask && !w.settled ? 1 : 0);
