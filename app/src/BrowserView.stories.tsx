// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A browser's view, its picture letterboxed in a pane larger than it**: the page is told as
 * 1280 by 800 while its frames are 1280 by 650, so the picture is laid out at the frame's own
 * shape, scaled down to the pane's width and centred, with a band above and below it. A click
 * on the picture reaches the page at the point the picture shows there, laid out by the real
 * engine (a point taken from the told size instead landed 69 px low, on what was under the
 * point the person aimed at).
 *
 * The frame is drawn here from the theme's own colours, so the picture is the same in every run.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, waitFor } from "storybook/test";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowserView } from "./BrowserView";
import type { BrowserFrame } from "./bindings";
import { forgetOrigins } from "./browserWatch";
import { forgetProjectsBrowsers, tellProjectBrowsers } from "./browsersGrid";
import type { Offer } from "./actions";
import { PaneActions, PaneActionsOf, PaneHead } from "./LayoutPanes";
import { paneLinesOf } from "./paneHead";
import { RECONNECTING_SAID } from "./reconnecting";
import { expectFocusInSight, untilFocusWithin, untilFocused } from "./stories.testkit";
import { ViewMark, ViewPane } from "./Views";

const PLANE = "/home/dev/plane";

/** The frame's own pixels: shorter than the 1280 by 800 the page is told as. */
const FRAME = { width: 1280, height: 650 };

/** The point of the page the story clicks: a mark is drawn there on the frame. */
const AIMED = { x: 640, y: 300 };

let frames: Channel<BrowserFrame> | undefined;
const inputs: { x: number; y: number }[] = [];

/** A frame of the page as a JPEG, in base64: the sunken ground, and a mark at {@link AIMED}. */
function framed(): string {
  const theme = getComputedStyle(document.documentElement);
  const drawn = document.createElement("canvas");
  drawn.width = FRAME.width;
  drawn.height = FRAME.height;
  const paint = drawn.getContext("2d");
  if (paint === null) throw new Error("no 2D context to draw the frame on");
  paint.fillStyle = theme.getPropertyValue("--surface-raised").trim();
  paint.fillRect(0, 0, FRAME.width, FRAME.height);
  paint.fillStyle = theme.getPropertyValue("--accent-base").trim();
  paint.fillRect(AIMED.x - 200, AIMED.y - 24, 400, 48);
  return drawn.toDataURL("image/jpeg", 0.9).replace(/^data:image\/jpeg;base64,/, "");
}

function Pane({ width = 900 }: { width?: number }) {
  return (
    <div className="view-pane" style={{ inlineSize: width, blockSize: 640 }}>
      <div className="view-body">
        <BrowserView plane={PLANE} program="B1" />
      </div>
    </div>
  );
}

/** The browser's view, as the window draws it in one side of a split (#6937): its pane frame,
 *  the pane head's grip and actions over the view's first row, and the view's own body. */
const BROWSER_VIEW = { from: null, view: "browser", key: "B1" } as const;

/** A view's pane's three controls, as the catalogue offers them. */
const PANE_ROWS: Offer[] = [
  {
    id: "pane.split.right",
    title: "Split right",
    available: true,
    reason: "",
    does: { verb: "split", direction: "row" },
  },
  {
    id: "pane.split.down",
    title: "Split down",
    available: true,
    reason: "",
    does: { verb: "split", direction: "column" },
  },
  {
    id: "pane.close",
    title: "Close this view",
    available: true,
    reason: "",
    does: { verb: "closePane" },
  },
];

function InAPane({ width }: { width: number }) {
  return (
    // A pane's size is its layout's (`container-type: size`), so the story gives it one.
    <div className="pane-frame pane-viewed" style={{ inlineSize: width, blockSize: 640 }}>
      <PaneActionsOf.Provider value={paneLinesOf("view", width, { alone: false }).actions}>
        <PaneHead
          lead={
            <span className="pane-view-grip">
              <ViewMark view={BROWSER_VIEW} />
            </span>
          }
          doing={
            <PaneActions
              pane={1}
              offerFor={(id) => PANE_ROWS.find((offer) => offer.id === id)}
              closeOf={() => PANE_ROWS[2]}
              onPaneDoes={() => undefined}
            />
          }
        />
      </PaneActionsOf.Provider>
      <div className="pane view focused">
        <ViewPane
          plane={PLANE}
          view={BROWSER_VIEW}
          title="Browser"
          waits={false}
          offered={[]}
          onOpenView={() => undefined}
          onAsk={() => undefined}
          onVaultChanged={() => undefined}
        />
      </div>
    </div>
  );
}

const meta = {
  component: Pane,
  beforeEach: () => {
    inputs.length = 0;
    frames = undefined;
    mockIPC((cmd, args) => {
      if (cmd === "chat_browsers") return [];
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<BrowserFrame> }).frames;
        return { view: 4, width: 1280, height: 800, person_drives: false, pool: null };
      }
      if (cmd === "take_over_browser") return null;
      if (cmd === "browser_input") {
        const { input } = args as { input: { x: number; y: number } };
        inputs.push({ x: input.x, y: input.y });
        return { outcome: "taken" };
      }
      return null;
    });
    return () => {
      clearMocks();
      forgetOrigins();
    };
  },
} satisfies Meta<typeof Pane>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Letterboxed**: the frame drawn at its own shape, centred with a band above and below, and
 * a click on the mark reaching the page at the mark. The keyboard reaches the address field.
 */
export const Letterboxed: Story = {
  play: async ({ canvas, userEvent }) => {
    await waitFor(() => expect(frames).toBeDefined());
    frames?.onmessage({
      aim: "p1:1",
      jpeg: framed(),
      reconnection: null,
      page: null,
      closed: null,
    });
    const picture = canvas.getByLabelText("The browser's page", { selector: "canvas" });
    await waitFor(() => expect(picture).toHaveProperty("height", FRAME.height));

    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "Address" }),
      "Tab reaches Address",
    );
    expectFocusInSight();
    (document.activeElement as HTMLElement | null)?.blur();

    // Where the engine laid the picture out: its box, the frame held whole inside it.
    const box = picture.getBoundingClientRect();
    const scale = Math.min(box.width / FRAME.width, box.height / FRAME.height);
    const top = box.top + (box.height - FRAME.height * scale) / 2;
    const left = box.left + (box.width - FRAME.width * scale) / 2;
    const surface = canvas.getByTestId("browser-surface");
    const band = top - surface.getBoundingClientRect().top;
    // The picture sits below a band of its pane's own ground, as the person saw it.
    await expect(band).toBeGreaterThan(20);

    const at = {
      clientX: left + AIMED.x * scale,
      clientY: top + AIMED.y * scale,
      button: 0,
      detail: 1,
      pointerId: 1,
      bubbles: true,
    };
    surface.dispatchEvent(new PointerEvent("pointerdown", at));
    surface.dispatchEvent(new PointerEvent("pointerup", at));
    await waitFor(() => expect(inputs).toHaveLength(2));
    await expect(inputs[0]).toEqual(AIMED);

    // The click took the browser over and left the keyboard on its page.
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** What the story's Go sent: the address, as the person typed it. */
const sent: string[] = [];

/**
 * **Handed over by its chat** (#6290): chat 3 asked the person, through `ask_operator`, to take
 * its browser over, and they drive it. Its view has the address field, so they can open the
 * address the chat gave them; the keyboard reaches it, Enter sends it, and the site new to the
 * project it held back waits as the chat's sites ask, drawn under the field with its answers.
 */
export const HandedOverByItsChat: Story = {
  beforeEach: () => {
    sent.length = 0;
    let asked = false;
    mockIPC((cmd, args) => {
      if (cmd === "chat_browsers")
        return [
          {
            session: 3,
            name: "b1",
            program: "B1",
            unended: false,
            pinned: false,
            was: null,
            login: null,
            driver: "person",
            pool: null,
            secrets: [],
          },
        ];
      if (cmd === "asks_waiting")
        return {
          plane: PLANE,
          asks: [
            {
              session: 3,
              ask: "browser-needs-you:B1",
              says: "Its browser waits for you on accounts.example.com",
              options: [{ id: "open-view", label: "Open its view", allows: false }],
              source: "browser-needs-you",
              chain: ["claude 3"],
              answer: { via: "take-over", program: "B1" },
              since: 10,
            },
            ...(asked
              ? [
                  {
                    session: 3,
                    ask: "browser-site:3",
                    says: "Its browser wants to reach example.com:443.",
                    options: [
                      { id: "chat", label: "Allow for this chat", allows: true },
                      { id: "keep", label: "Keep blocked", allows: false },
                    ],
                    source: "browser-site",
                    chain: ["claude 3"],
                    answer: { via: "browser-sites", sites: ["example.com:443"] },
                    since: 11,
                  },
                ]
              : []),
          ],
        };
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<BrowserFrame> }).frames;
        return { view: 4, width: 1280, height: 800, person_drives: true, pool: null };
      }
      if (cmd === "go_browser") {
        sent.push((args as { address: string }).address);
        asked = true;
        return { outcome: "asks", sites: ["example.com:443"], answers: [] };
      }
      return null;
    });
  },
  play: async ({ canvas, userEvent }) => {
    await waitFor(() => expect(frames).toBeDefined());
    frames?.onmessage({
      aim: "p1:1",
      jpeg: framed(),
      reconnection: null,
      page: null,
      closed: null,
    });

    await userEvent.tab();
    const address = await waitFor(() => canvas.getByRole("textbox", { name: "Address" }));
    await untilFocused(() => address, "Tab reaches Address");
    expectFocusInSight();
    await userEvent.keyboard("example.com{Enter}");
    await waitFor(() => expect(sent).toEqual(["example.com"]));

    const allow = await waitFor(() => canvas.getByRole("button", { name: "Allow for this chat" }), {
      timeout: 5000,
    });
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Go" }), "Tab reaches Go");
    expectFocusInSight();
    await expect(allow).toBeVisible();
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** The keys of the person's screenshot (#6937): long, and more of them than a row holds. */
const KEYS = [
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
  "STAGE_QA_S3RACE_RUNNER_KUBECONFIG_WITH_A_LONG_SUFFIX",
];

/** A chat's browser, steward 4's, with `keys` filled on `origin`, its agent or the person
 *  driving it. */
function chatsBrowser(origin: string, keys: readonly string[], personDrives = false) {
  return () => {
    tellProjectBrowsers({
      plane: PLANE,
      project: "plane",
      tiles: [
        {
          plane: PLANE,
          program: "B1",
          own: false,
          name: "b1",
          chat: "steward 4",
          workspace: "",
          driver: null,
          pool: null,
        },
      ],
      open: () => {},
    });
    mockIPC((cmd, args) => {
      if (cmd === "chat_browsers")
        return [
          {
            session: 4,
            name: "b1",
            program: "B1",
            unended: false,
            pinned: false,
            was: null,
            login: null,
            driver: personDrives ? "person" : null,
            pool: null,
            secrets: keys.map((key) => ({ key, origin, page: origin, label: "", code: false })),
          },
        ];
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<BrowserFrame> }).frames;
        return { view: 4, width: 1280, height: 800, person_drives: personDrives, pool: null };
      }
      return null;
    });
    return () => forgetProjectsBrowsers();
  };
}

/** Draws a frame on `origin`, and waits until the row names it. */
async function onPage(origin: string, host: string) {
  await waitFor(() => expect(frames).toBeDefined());
  frames?.onmessage({ aim: "", jpeg: "", reconnection: null, page: { origin }, closed: null });
  frames?.onmessage({ aim: "p1:1", jpeg: framed(), reconnection: null, page: null, closed: null });
  await waitFor(() =>
    expect(document.querySelector('[data-testid="browser-chip"]')).toHaveTextContent(host),
  );
}

/** The header's rows fit the pane: none is wider than it, and neither is taller than two lines.
 *  In a pane frame, nothing of the first row is under the pane head's grip or its actions. */
async function fitsThePane(
  canvas: { getByTestId: (id: string) => HTMLElement },
  /** The strip's tallest: two lines and its controls, unless a story has it say more. */
  barAtMost = 96,
) {
  const head = canvas.getByTestId("browser-head");
  const frame = head.closest(".pane-frame");
  if (frame !== null) {
    const kept = [
      frame.querySelector(".pane-head > .pane-view-grip"),
      frame.querySelector(".pane-head .pane-actions"),
    ].flatMap((one) => (one === null ? [] : [one.getBoundingClientRect()]));
    await expect(kept, "the pane head's grip and its actions are drawn").toHaveLength(2);
    for (const part of head.children) {
      const box = part.getBoundingClientRect();
      if (box.width === 0) continue;
      for (const under of kept) {
        const overlaps = box.left < under.right && box.right > under.left;
        await expect(
          overlaps && box.top < under.bottom && box.bottom > under.top,
          `${part.className} is clear of the pane head's controls`,
        ).toBe(false);
      }
    }
  }
  // What each part of the row took, said where the row runs past the pane.
  const parts = [...head.querySelectorAll<HTMLElement>("*")]
    .filter((one) => one.getBoundingClientRect().right > head.getBoundingClientRect().right)
    .map(
      (one) => `${one.tagName}.${one.className} ${Math.round(one.getBoundingClientRect().width)}`,
    )
    .join("; ");
  await expect(head.scrollWidth, `the row's parts past it: ${parts}`).toBeLessThanOrEqual(
    head.clientWidth,
  );
  await expect(head.getBoundingClientRect().height).toBeLessThan(64);
  const bar = head.parentElement?.querySelector<HTMLElement>(".browser-bar");
  await expect(bar?.getBoundingClientRect().height ?? 0).toBeLessThan(barAtMost);
  // The strip wraps rather than cutting its words or running past the pane.
  await expect(bar?.scrollWidth ?? 0).toBeLessThanOrEqual(bar?.clientWidth ?? 0);
  await expect(bar?.scrollHeight ?? 0).toBeLessThanOrEqual(bar?.clientHeight ?? 0);
}

/**
 * **The person's screenshot, drawn again** (#6937): steward 4 drives its browser on a secure
 * site where purlis filled three long keys. The site, the lock and a count of the keys are the
 * view's first row; the strip says who drives and offers Take control. The keyboard reaches the
 * count, whose list of keys Escape closes, then Take control.
 */
export const ItsChatDrivesWithSecrets: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(0, 3)),
  play: async ({ canvas, userEvent }) => {
    await onPage("https://console.volaticloud.com", "console.volaticloud.com");
    const count = await waitFor(() => canvas.getByRole("button", { name: "3 secrets" }));
    await fitsThePane(canvas);
    await expect(canvas.getByRole("status")).toHaveTextContent("steward 4 is using this browser.");

    count.focus();
    await userEvent.keyboard("{Enter}");
    const list = await screen.findByRole("dialog", { name: "Secrets purlis filled on this site" });
    await untilFocusWithin(() => list, "Enter opens the keys filled");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await untilFocused(() => count, "Escape gives the keyboard back to the count");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Take control" }),
      "Tab reaches Take control",
    );
    expectFocusInSight();
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/**
 * **A narrow pane, a long host on plain http and a dozen keys** (#6937): the host is cut, Not
 * secure stays, the keys are one count, and the strip's controls go under its sentence; nothing
 * runs past the pane.
 */
export const ANarrowPane: Story = {
  args: { width: 420 },
  beforeEach: chatsBrowser(
    "http://a-very-long-preview-host-for-pull-request-6910.staging.internal.example.com:8080",
    KEYS,
  ),
  play: async ({ canvas }) => {
    await onPage(
      "http://a-very-long-preview-host-for-pull-request-6910.staging.internal.example.com:8080",
      "Not secure",
    );
    const count = await waitFor(() => canvas.getByRole("button", { name: "12 secrets" }));
    await fitsThePane(canvas);
    // The site is cut before the count is: the count is whole.
    await expect(count.scrollWidth).toBeLessThanOrEqual(count.clientWidth);
    const head = canvas.getByTestId("browser-head").getBoundingClientRect();
    await expect(count.getBoundingClientRect().right).toBeLessThanOrEqual(head.right);
  },
};

/** **The person drives** (#6937): the strip on the accent says the agent is paused, with Done. */
export const YouDrive: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(1, 2), true),
  play: async ({ canvas }) => {
    await onPage("https://console.volaticloud.com", "console.volaticloud.com");
    await waitFor(() => canvas.getByRole("button", { name: /Done/ }));
    await expect(canvas.getByRole("status")).toHaveTextContent(
      "You are driving this browser. The agent is paused until you press Done.",
    );
    await expect(
      canvas.getByRole("img", {
        name: "Secret STAGE_S3_ACCESS_KEY, filled on console.volaticloud.com",
      }),
    ).toBeVisible();
    await fitsThePane(canvas);
  },
};

/**
 * **In a real pane, 420px wide** (#6937): one side of a split, as the window draws it. The view's
 * first row is the pane head's row, so the site and the count of keys sit between the pane's
 * grip and its three controls, under neither, and nothing runs past the pane.
 */
export const InANarrowSplitPane: Story = {
  render: () => <InAPane width={420} />,
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(0, 3)),
  play: async ({ canvas }) => {
    await onPage("https://console.volaticloud.com", "console.volaticloud.com");
    const count = await waitFor(() => canvas.getByRole("button", { name: "3 secrets" }));
    await fitsThePane(canvas);
    await expect(count.scrollWidth).toBeLessThanOrEqual(count.clientWidth);
    await expect(canvas.getByRole("button", { name: "Split right" })).toBeVisible();
  },
};

/**
 * **In a real pane, 420px wide, with one long key** (#6937): one secret keeps its chip, cut
 * with an ellipsis before it reaches the pane's controls.
 */
export const InANarrowSplitPaneWithOneKey: Story = {
  render: () => <InAPane width={420} />,
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(-1)),
  play: async ({ canvas }) => {
    await onPage("https://console.volaticloud.com", "console.volaticloud.com");
    const chip = await waitFor(() => canvas.getByTestId("secret-mark"));
    await fitsThePane(canvas);
    // The chip shrinks within the row, its rounded ends drawn, and its words end in an ellipsis
    // rather than being clipped mid-letter by the row.
    const group = chip.parentElement?.getBoundingClientRect();
    await expect(chip.getBoundingClientRect().right).toBeLessThanOrEqual(group?.right ?? 0);
    const said = chip.querySelector<HTMLElement>(".browser-secret-said");
    await expect(said?.scrollWidth ?? 0).toBeGreaterThan(said?.clientWidth ?? 0);
    await expect(said === null ? "" : getComputedStyle(said).textOverflow).toBe("ellipsis");
  },
};

/**
 * **Everything the strip has to say at once, in a narrow pane** (#6937): its runner's link is
 * down, a key the person typed was dropped and the core refused another. Each is said in the
 * strip, which wraps under its sentence rather than running past the pane.
 */
export const ANarrowPaneSayingEverything: Story = {
  args: { width: 420 },
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "chat_browsers") return [];
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<BrowserFrame> }).frames;
        return { view: 4, width: 1280, height: 800, person_drives: true, pool: "build-pool" };
      }
      if (cmd === "browser_input") {
        const { input } = args as { input: { kind: string; action: string } };
        // The key's press reaches a page that moved on; its release is refused.
        if (input.kind === "key" && input.action === "down") return { outcome: "missed" };
        return { outcome: "refused", why: "The browser's runner did not take the key." };
      }
      return null;
    });
  },
  play: async ({ canvas, userEvent }) => {
    await waitFor(() => expect(frames).toBeDefined());
    frames?.onmessage({
      aim: "p1:1",
      jpeg: framed(),
      reconnection: null,
      page: null,
      closed: null,
    });
    await waitFor(() => canvas.getByRole("button", { name: /Done/ }));
    const status = canvas.getByRole("status");

    canvas.getByTestId("browser-surface").focus();
    await userEvent.keyboard("a");
    await waitFor(() => expect(status).toHaveTextContent("The page changed before"));
    await waitFor(() => expect(status).toHaveTextContent("did not take the key"));
    frames?.onmessage({
      aim: "",
      jpeg: "",
      reconnection: "reconnecting",
      page: null,
      closed: null,
    });
    await waitFor(() => expect(status).toHaveTextContent(RECONNECTING_SAID));
    await expect(status).toHaveTextContent("The page changed before");
    await expect(status).toHaveTextContent("did not take the key");
    // Three things said at once wrap to more lines than one, every word of them whole.
    await fitsThePane(canvas, 160);
    (document.activeElement as HTMLElement | null)?.blur();
  },
};
