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
import type { BrowserFrame, ShownPage } from "./bindings";
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
      shown: null,
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
      shown: null,
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
  frames?.onmessage({
    aim: "",
    jpeg: "",
    reconnection: null,
    page: { origin },
    closed: null,
    shown: null,
  });
  frames?.onmessage({
    aim: "p1:1",
    jpeg: framed(),
    reconnection: null,
    page: null,
    closed: null,
    shown: null,
  });
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
  // Nothing runs into the row's padding at its end, where the row would clip it: a scroll width
  // counts that padding as room.
  const end = head.getBoundingClientRect().right - parseFloat(getComputedStyle(head).paddingRight);
  for (const part of head.children) {
    await expect(
      part.getBoundingClientRect().right,
      `${part.className} ends inside the row`,
    ).toBeLessThanOrEqual(end + 0.5);
  }
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
      shown: null,
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
      shown: null,
    });
    await waitFor(() => expect(status).toHaveTextContent(RECONNECTING_SAID));
    await expect(status).toHaveTextContent("The page changed before");
    await expect(status).toHaveTextContent("did not take the key");
    // Three things said at once wrap to more lines than one, every word of them whole.
    await fitsThePane(canvas, 160);
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** Tells the view what its page is (#6938), as the core does, after a frame on `origin`. */
async function told(origin: string, host: string, shown: ShownPage) {
  await onPage(origin, host);
  frames?.onmessage({
    aim: "",
    jpeg: "",
    reconnection: null,
    page: null,
    closed: null,
    shown,
  });
}

/** The page of the stories of what a page is (#6938). */
const DOCS: ShownPage = {
  address: "https://console.volaticloud.com/projects/fleet/runners?tab=pending",
  title: "Pending runners · Fleet · VolatiCloud console",
  load: { state: "loaded" },
  pages: [],
  downloads: [],
};

/**
 * **What the page is** (#6938): its title after the site, Copy for its whole address, and Back,
 * Forward and Reload at the strip's start. The keyboard goes Back, Forward, Reload, then the
 * strip's Take control.
 */
export const ItsTitleAndHistory: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas, userEvent }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", DOCS);
    await waitFor(() =>
      expect(canvas.getByTestId("browser-title")).toHaveTextContent("Pending runners"),
    );
    await expect(canvas.getByRole("button", { name: "Copy address" })).toBeVisible();
    await fitsThePane(canvas);

    const back = canvas.getByRole("button", { name: "Back" });
    back.focus();
    await untilFocused(() => back, "Back takes the keyboard");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Forward" }),
      "Tab reaches Forward",
    );
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Reload" }), "Tab reaches Reload");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Take control" }),
      "Tab reaches Take control",
    );
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** **A page loading** (#6938): the strip's lower edge moves, it says so, and Stop is in Reload's
 *  place. */
export const APageLoading: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...DOCS,
      load: { state: "loading" },
    });
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Loading the page…"));
    await expect(canvas.getByRole("button", { name: "Stop" })).toBeVisible();
    await expect(
      document.querySelector(".browser-bar")?.hasAttribute("data-loading"),
      "the strip draws the load on its lower edge",
    ).toBe(true);
    await fitsThePane(canvas);
  },
};

/** **A page that did not load, in a narrow pane** (#6938): the strip says so with the engine's
 *  reason and what to do, and wraps rather than running past the pane. */
export const APageThatDidNotLoad: Story = {
  args: { width: 420 },
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...DOCS,
      title: null,
      load: { state: "failed", why: "net::ERR_NAME_NOT_RESOLVED" },
    });
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "This page did not load (net::ERR_NAME_NOT_RESOLVED). Press Reload to try again.",
      ),
    );
    await expect(canvas.getByRole("button", { name: "Reload" })).toBeVisible();
    await fitsThePane(canvas, 160);
  },
};

/** **A page that stopped working** (#6938): its renderer crashed, and the strip says Reload. */
export const APageThatStoppedWorking: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...DOCS,
      load: { state: "crashed" },
    });
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "This page stopped working. Press Reload to open it again.",
      ),
    );
    await fitsThePane(canvas);
  },
};

/**
 * **A long title in a real pane, 420px wide** (#6938): the title is cut before the site is, and
 * Copy, the site and the pane head's controls keep their room; nothing runs past the pane.
 */
export const ALongTitleInANarrowSplitPane: Story = {
  render: () => <InAPane width={420} />,
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(0, 3)),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...DOCS,
      title: `${"A very long page title that goes on ".repeat(6)}and on`,
    });
    const title = await waitFor(() => canvas.getByTestId("browser-title"));
    await fitsThePane(canvas);
    await expect(title.scrollWidth).toBeGreaterThan(title.clientWidth);
    await expect(canvas.getByRole("button", { name: "Copy address" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "3 secrets" })).toBeVisible();
  },
};

/** The pages of the stories of a browser's other pages (#6989): its chat opened three. */
const THREE_PAGES: ShownPage = {
  ...DOCS,
  pages: [
    {
      id: "p1",
      title: "Fleet · VolatiCloud console",
      origin: "https://console.volaticloud.com",
      current: false,
    },
    { id: "p2", title: DOCS.title, origin: "https://console.volaticloud.com", current: true },
    { id: "p3", title: null, origin: "https://id.volaticloud.com", current: false },
  ],
};

/**
 * **A browser with several pages** (#6989): "Page 2 of 3" after the title, whose menu lists each
 * page with its site and ticks the one shown. The keyboard: Enter opens it on its rows, Escape
 * gives the keyboard back to it.
 */
export const ItsOtherPages: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas, userEvent }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", THREE_PAGES);
    const pages = await waitFor(() => canvas.getByRole("button", { name: "Page 2 of 3" }));
    await fitsThePane(canvas);
    pages.focus();
    await untilFocused(() => pages, "the count takes the keyboard");
    await userEvent.keyboard("{Enter}");
    // The menu fades in as it opens: it is waited for until it is drawn.
    await waitFor(() =>
      expect(screen.getByRole("menu", { name: "This browser's pages" })).toBeVisible(),
    );
    await expect(screen.getByRole("menuitemradio", { name: DOCS.title ?? "" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => pages, "Escape gives the keyboard back to the count");
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** **Its pages' menu, open** (#6989): each page by its title and site, one by its site alone. */
export const ItsPagesMenuOpen: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas, userEvent }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", THREE_PAGES);
    await userEvent.click(await waitFor(() => canvas.getByRole("button", { name: "Page 2 of 3" })));
    await waitFor(() => expect(screen.getAllByTestId("browser-page")).toHaveLength(3));
    // The menu fades in as it opens: its rows are waited for until they are drawn.
    await waitFor(() =>
      expect(screen.getByRole("menuitemradio", { name: "id.volaticloud.com" })).toBeVisible(),
    );
  },
};

/**
 * **Several pages, a long title and three secrets in a real pane, 420px wide** (#6989): the title
 * gives up its room first, and the count, Copy and the secrets stay whole.
 */
export const ItsPagesInANarrowSplitPane: Story = {
  render: () => <InAPane width={420} />,
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(0, 3)),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...THREE_PAGES,
      title: `${"A very long page title that goes on ".repeat(6)}and on`,
    });
    const count = await waitFor(() => canvas.getByRole("button", { name: "Page 2 of 3" }));
    await fitsThePane(canvas);
    await expect(count.scrollWidth).toBeLessThanOrEqual(count.clientWidth);
    await expect(canvas.getByRole("button", { name: "3 secrets" })).toBeVisible();
  },
};

/** The downloads of the stories of a browser's downloads (#6990): one under way, one done, and
 *  one purlis cancelled for its size. */
const THREE_DOWNLOADS: ShownPage = {
  ...DOCS,
  downloads: [
    {
      id: "d1",
      name: "runners-2026-11.csv",
      state: "going",
      received: 1_250_000,
      total: 3_400_000,
    },
    { id: "d2", name: "invoice-1042.pdf", state: "done", received: 84_000, total: 84_000 },
    { id: "d3", name: "fleet-backup.tar.gz", state: "too_large", received: 0, total: null },
  ],
};

/**
 * **A browser whose pages downloaded files** (#6990): "3 downloads" in the first row. The
 * keyboard: Enter opens its list, Escape gives the keyboard back to it.
 */
export const ItsDownloads: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas, userEvent }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", THREE_DOWNLOADS);
    const count = await waitFor(() => canvas.getByRole("button", { name: "3 downloads" }));
    await fitsThePane(canvas);
    count.focus();
    await untilFocused(() => count, "the count takes the keyboard");
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.getByRole("dialog", { name: "This browser's downloads" })).toBeVisible(),
    );
    await userEvent.keyboard("{Escape}");
    await untilFocused(() => count, "Escape gives the keyboard back to the count");
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

/** **Its downloads' list, open** (#6990): each by its name, with how far it came under it. A
 *  name holding a character a reader would not see shows it marked (#2020). */
export const ItsDownloadsOpen: Story = {
  beforeEach: chatsBrowser("https://console.volaticloud.com", []),
  play: async ({ canvas, userEvent }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...THREE_DOWNLOADS,
      downloads: [
        ...THREE_DOWNLOADS.downloads,
        { id: "d4", name: "report\u202Efdp.exe", state: "cancelled", received: 0, total: null },
      ],
    });
    await userEvent.click(await waitFor(() => canvas.getByRole("button", { name: "4 downloads" })));
    await waitFor(() => expect(screen.getAllByTestId("browser-download")).toHaveLength(4));
    await expect(screen.getAllByTestId("browser-download")[3]).toHaveTextContent(
      "report⟦U+202E⟧fdp.exe",
    );
  },
};

/**
 * **Pages, downloads, a long title and three secrets in a real pane, 420px wide** (#6990): the
 * title gives up its room first, and every count stays whole.
 */
export const ItsDownloadsInANarrowSplitPane: Story = {
  render: () => <InAPane width={420} />,
  beforeEach: chatsBrowser("https://console.volaticloud.com", KEYS.slice(0, 3)),
  play: async ({ canvas }) => {
    await told("https://console.volaticloud.com", "console.volaticloud.com", {
      ...THREE_PAGES,
      downloads: THREE_DOWNLOADS.downloads,
      title: `${"A very long page title that goes on ".repeat(6)}and on`,
    });
    const count = await waitFor(() => canvas.getByRole("button", { name: "3 downloads" }));
    await fitsThePane(canvas);
    await expect(count.scrollWidth).toBeLessThanOrEqual(count.clientWidth);
    await expect(canvas.getByRole("button", { name: "Page 2 of 3" })).toBeVisible();
  },
};
