// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE, never merged** (`prototype/browser-view-header`): three ways to draw the header
 * above a watched browser's page, each in every state the design note lists
 * (`docs/design/browser-view-header.md`), at a wide (900px) and a narrow (420px) pane. The story
 * matrix draws each in every theme, appearance and density. Built from the window's families and
 * theme tokens; the states are data, nothing runs.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  Bug,
  Columns2,
  GripVertical,
  Hand,
  KeyRound,
  Lock,
  MoreHorizontal,
  RotateCw,
  Rows2,
  ShieldAlert,
  WifiOff,
  X,
} from "lucide-react";
import { expect, screen, waitFor } from "storybook/test";
import { Button, IconButton, ToggleButton } from "./ui/button/Button";
import { Menu, MenuItem, MenuTrigger } from "./ui/menu/Menu";
import { PopoverDialog, PopoverTrigger } from "./ui/popover/Popover";
import { Select } from "./ui/select/Select";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { expectFocusInSight, untilFocusWithin, untilFocused } from "./stories.testkit";
import "./BrowserHeader.prototype.css";

type Driver = "agent" | "person" | "purlis" | "theirs";

/** One state the header is drawn in. */
type Shown = {
  case: string;
  origin: string | null;
  device?: string;
  driver: Driver;
  chat?: string;
  secrets?: string[];
  trouble?: string;
  reconnecting?: boolean;
  /** What only the core can tell yet (the design note's "core" rows). */
  title?: string;
  loading?: boolean;
  nav?: boolean;
};

const LONG_KEY = "STAGE_QA_S3RACE_RUNNER_KUBECONFIG_WITH_A_LONG_SUFFIX";
const MANY = [
  "STAGE_QA_S3RACE_RUNNER_KUBECONFIG",
  "STAGE_S3_ACCESS_KEY",
  "STAGE_S3_SECRET_KEY",
  "DASH_ADMIN_PASSWORD",
  "DASH_ADMIN_TOTP",
  "GRAFANA_TOKEN",
  "PAGERDUTY_TOKEN",
  "SENTRY_DSN",
  "STRIPE_TEST_KEY",
  "VULTR_API_KEY",
  "SLACK_BOT_TOKEN",
  LONG_KEY,
];

const STATES: Shown[] = [
  { case: "No page yet", origin: null, driver: "agent", chat: "steward 4" },
  {
    case: "A chat drives, one secret filled",
    origin: "https://console.volaticloud.com",
    driver: "agent",
    chat: "steward 4",
    secrets: ["STAGE_S3_ACCESS_KEY"],
  },
  {
    case: "The screenshot's case: three long keys",
    origin: "https://console.volaticloud.com",
    driver: "agent",
    chat: "steward 4",
    secrets: MANY.slice(0, 3),
  },
  {
    case: "You drive, twelve secrets",
    origin: "https://console.volaticloud.com",
    driver: "person",
    chat: "steward 4",
    secrets: MANY,
  },
  {
    case: "purlis signs in",
    origin: "https://accounts.example.com",
    driver: "purlis",
    chat: "steward 4",
  },
  {
    case: "Not secure, on a runner, very long host",
    origin:
      "http://a-very-long-preview-host-name-for-a-pull-request-6910-of-the-dashboard.staging.internal.example.com:8080",
    device: "linux-pool",
    driver: "agent",
    chat: "release-bot 12",
  },
  {
    case: "Reconnecting",
    origin: "https://dash.example.test",
    device: "linux-pool",
    driver: "agent",
    chat: "steward 4",
    reconnecting: true,
  },
  {
    case: "Your key was dropped",
    origin: "https://dash.example.test",
    driver: "person",
    chat: "steward 4",
    trouble: "The page changed before your key or click reached it. Type or click again.",
  },
  { case: "Your own browser", origin: "https://example.com", driver: "theirs" },
  {
    case: "Later, once the core tells: title, navigation, loading",
    origin: "https://console.volaticloud.com",
    driver: "agent",
    chat: "steward 4",
    title: "QA S3 race (#4466) · Runners · VolatiCloud",
    loading: true,
    nav: true,
    secrets: ["STAGE_S3_ACCESS_KEY"],
  },
];

function hostOf(origin: string): string {
  try {
    return new URL(origin).host;
  } catch {
    return origin;
  }
}

/** The pane head's grip and the pane's actions, as the window draws them around a view. */
function Grip() {
  return (
    <span className="bhp-grip">
      <IconButton size="sm" variant="ghost" aria-label="Move this pane" icon={<GripVertical />} />
    </span>
  );
}

function PaneActions() {
  return (
    <span className="bhp-actions">
      <IconButton size="sm" variant="ghost" aria-label="Split right" icon={<Columns2 />} />
      <IconButton size="sm" variant="ghost" aria-label="Split down" icon={<Rows2 />} />
      <IconButton size="sm" variant="ghost" aria-label="Close pane" icon={<X />} />
    </span>
  );
}

/** The site: lock and host, "Not secure" for http, "No page yet" before one. */
function Site({ shown }: { shown: Shown }) {
  if (shown.origin === null)
    return (
      <span className="bhp-site">
        <span className="bhp-site-device">No page yet</span>
      </span>
    );
  const secure = shown.origin.startsWith("https://");
  return (
    <Tooltip tip={shown.origin}>
      <span className="bhp-site" data-testid="site">
        {secure ? (
          <Lock aria-label="Secure connection" />
        ) : (
          <ShieldAlert className="bhp-insecure" aria-label="Not secure" />
        )}
        {!secure && <span className="bhp-insecure bhp-wide-only">Not secure</span>}
        <span className="bhp-site-host">{hostOf(shown.origin)}</span>
        {shown.device !== undefined && (
          <span className="bhp-site-device">on {shown.device}</span>
        )}
        {shown.title !== undefined && (
          <span className="bhp-site-title bhp-wide-only">· {shown.title}</span>
        )}
      </span>
    </Tooltip>
  );
}

/** The secrets purlis filled: one chip when it is one, else a count opening the list. Names
 *  only, never a value. */
function Secrets({ keys, origin }: { keys: string[]; origin: string | null }) {
  if (keys.length === 0 || origin === null) return null;
  const site = hostOf(origin);
  const list = (
    <ul className="bhp-secret-list">
      {keys.map((key) => (
        <li key={key}>
          <span>{key}</span>
          <small>Filled on {site}. The chat never saw the value.</small>
        </li>
      ))}
    </ul>
  );
  return (
    <span className="bhp-secrets">
      <PopoverTrigger>
        <Button size="sm" variant="ghost" icon={<KeyRound />}>
          {keys.length === 1 ? keys[0] : `${keys.length} secrets`}
        </Button>
        <PopoverDialog aria-label="Secrets purlis filled on this site">{list}</PopoverDialog>
      </PopoverTrigger>
    </span>
  );
}

function SizeSelect({ narrow }: { narrow: boolean }) {
  return (
    <Select
      size="sm"
      aria-label="Page size"
      value="laptop"
      options={[
        { value: "laptop", label: narrow ? "Laptop" : "Laptop, 1280 × 800" },
        { value: "phone", label: narrow ? "Phone" : "Phone, 390 × 844" },
      ]}
      onValueChange={() => {}}
    />
  );
}

function Nav() {
  return (
    <>
      <IconButton size="sm" variant="ghost" aria-label="Back" icon={<ArrowLeft />} />
      <IconButton size="sm" variant="ghost" aria-label="Forward" icon={<ArrowRight />} />
      <IconButton size="sm" variant="ghost" aria-label="Reload" icon={<RotateCw />} />
    </>
  );
}

/** Who drives, as a sentence, and its short form for a narrow pane. */
function driverSaid(shown: Shown): { long: string; short: string; who?: string } {
  const chat = shown.chat ?? "The chat";
  switch (shown.driver) {
    case "agent":
      return {
        who: chat,
        long: " is using this browser. Click the page or Take control to use it yourself.",
        short: " is using it",
      };
    case "person":
      return {
        long: `You're in control. ${chat} is paused until you hand it back.`,
        short: "You're in control",
      };
    case "purlis":
      return {
        long: "purlis is signing in for the chat. Wait for it to finish.",
        short: "purlis is signing in",
      };
    case "theirs":
      return { long: "Your browser. Type an address or click the page.", short: "Your browser" };
  }
}

function DriveButton({ shown }: { shown: Shown }) {
  if (shown.driver === "agent")
    return (
      <Button size="sm" icon={<Hand />} tooltip="The chat pauses while you drive">
        Take control
      </Button>
    );
  if (shown.driver === "person")
    return (
      <Button size="sm" variant="primary" icon={<Bot />} shortcut="⌘⇧↩">
        Hand back
      </Button>
    );
  return null;
}

/** **B**: identity on the pane head's row, and a strip that says who drives. */
function VariantB({ shown, narrow }: { shown: Shown; narrow: boolean }) {
  const said = driverSaid(shown);
  return (
    <>
      <div className="bhp-row">
        <Grip />
        <Site shown={shown} />
        <Secrets keys={shown.secrets ?? []} origin={shown.origin} />
        <span className="bhp-fill" />
        <PaneActions />
      </div>
      <div className="bhp-strip" data-driving={shown.driver === "person" ? "person" : "agent"}>
        {shown.nav && <Nav />}
        {shown.reconnecting ? (
          <WifiOff aria-hidden="true" />
        ) : shown.driver === "person" ? (
          <Hand aria-hidden="true" />
        ) : (
          <Bot aria-hidden="true" />
        )}
        <span className="bhp-said" role="status">
          {shown.reconnecting ? (
            <>
              <strong>Reconnecting…</strong>
              <span className="bhp-said-long"> The runner's link is down; the page waits.</span>
            </>
          ) : (
            <>
              {said.who !== undefined && <strong>{said.who}</strong>}
              <span className="bhp-said-long">{said.long}</span>
              <span className="bhp-said-short">{said.short}</span>
            </>
          )}
          {shown.trouble !== undefined && (
            <span className="bhp-said-trouble"> {shown.trouble}</span>
          )}
        </span>
        <span className="bhp-wide-only">
          <SizeSelect narrow={narrow} />
        </span>
        {!narrow && shown.device === undefined && (
          <IconButton size="sm" variant="ghost" aria-label="Inspect" icon={<Bug />} />
        )}
        <DriveButton shown={shown} />
      </div>
      {shown.loading && <div className="bhp-progress" aria-label="Loading" role="progressbar" />}
    </>
  );
}

/** **A**: one row, a status line under it only when there is something to say. */
function VariantA({ shown }: { shown: Shown; narrow: boolean }) {
  const said = driverSaid(shown);
  return (
    <>
      <div className="bhp-row">
        <Grip />
        {shown.nav && <Nav />}
        <Site shown={shown} />
        <Secrets keys={shown.secrets ?? []} origin={shown.origin} />
        <span className="bhp-fill" />
        <span
          className="bhp-pill bhp-wide-only"
          data-driving={shown.driver === "person" ? "person" : "agent"}
        >
          {shown.driver === "person" ? <Hand aria-hidden="true" /> : <Bot aria-hidden="true" />}
          <span>
            {said.who}
            {said.short}
          </span>
        </span>
        <DriveButton shown={shown} />
        <MenuTrigger>
          <IconButton size="sm" variant="ghost" aria-label="More" icon={<MoreHorizontal />} />
          <Menu aria-label="More">
            <MenuItem id="size">Page size…</MenuItem>
            <MenuItem id="inspect">Inspect</MenuItem>
          </Menu>
        </MenuTrigger>
        <PaneActions />
      </div>
      {(shown.reconnecting || shown.trouble !== undefined) && (
        <p className="bhp-line" role="status">
          {shown.reconnecting ? "Reconnecting…" : shown.trouble}
        </p>
      )}
      {shown.loading && <div className="bhp-progress" aria-label="Loading" role="progressbar" />}
    </>
  );
}

/** **C**: a browser's omnibox and a Watch | Control toggle. */
function VariantC({ shown, narrow }: { shown: Shown; narrow: boolean }) {
  const person = shown.driver === "person" || shown.driver === "theirs";
  return (
    <>
      <div className="bhp-row">
        <Grip />
        <span className="bhp-fill" />
        <PaneActions />
      </div>
      <div className="bhp-row bhp-under">
        {shown.nav && <Nav />}
        <span className="bhp-omni">
          <Site shown={shown} />
          <span className="bhp-fill" />
          <Secrets keys={shown.secrets ?? []} origin={shown.origin} />
        </span>
        {shown.driver !== "theirs" && (
          <span className="bhp-grip" role="group" aria-label="Who drives">
            <ToggleButton size="sm" isSelected={!person} icon={<Bot />}>
              {narrow ? "" : "Watch"}
            </ToggleButton>
            <ToggleButton size="sm" isSelected={person} icon={<Hand />}>
              {narrow ? "" : "Control"}
            </ToggleButton>
          </span>
        )}
      </div>
      {(shown.reconnecting || shown.trouble !== undefined) && (
        <p className="bhp-line" role="status">
          {shown.reconnecting ? "Reconnecting…" : shown.trouble}
        </p>
      )}
      {shown.loading && <div className="bhp-progress" aria-label="Loading" role="progressbar" />}
    </>
  );
}

const VARIANTS = { A: VariantA, B: VariantB, C: VariantC } as const;

function Board({ variant, only }: { variant: keyof typeof VARIANTS; only?: number[] }) {
  const Drawn = VARIANTS[variant];
  const states = only === undefined ? STATES : only.map((one) => STATES[one]);
  return (
    <div className="bhp-board">
      {states.map((shown) => (
        <section key={shown.case} className="bhp-case" aria-label={shown.case}>
          <h3>{shown.case}</h3>
          <div className="bhp-widths">
            {[900, 420].map((width) => (
              <div
                key={width}
                className="bhp-pane"
                style={{ inlineSize: width }}
                data-narrow={width < 600 ? "" : undefined}
              >
                <Drawn shown={shown} narrow={width < 600} />
                <div className="bhp-page">The page</div>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

const meta = {
  component: Board,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Board>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A, one toolbar row.** */
export const VariantAOneRow: Story = { args: { variant: "A" } };

/** **B, identity row and a driving strip** (chosen). The keyboard reaches the secrets' count,
 *  which opens the list of keys, and Escape gives it back. */
export const VariantBIdentityAndStrip: Story = {
  args: { variant: "B" },
  play: async ({ canvas, userEvent }) => {
    const count = canvas.getAllByRole("button", { name: "12 secrets" })[0];
    count.focus();
    await userEvent.keyboard("{Enter}");
    const list = await screen.findByRole("dialog", { name: "Secrets purlis filled on this site" });
    await untilFocusWithin(() => list, "Enter opens the list of keys");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await untilFocused(() => count, "Escape gives the keyboard back to the count");
    expectFocusInSight();
  },
};

/** **C, browser-style omnibox.** */
export const VariantCOmnibox: Story = { args: { variant: "C" } };

/** **Before and after, the screenshot's case**: B with three long keys and twelve. */
export const ChosenScreenshotCase: Story = { args: { variant: "B", only: [2, 3, 5] } };
