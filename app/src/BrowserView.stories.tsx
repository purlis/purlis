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
import { expect, waitFor } from "storybook/test";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowserView } from "./BrowserView";
import type { BrowserFrame } from "./bindings";
import { forgetOrigins } from "./browserWatch";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

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

function Pane() {
  return (
    <div className="view-pane" style={{ inlineSize: 900, blockSize: 640 }}>
      <div className="view-body">
        <BrowserView plane={PLANE} program="B1" />
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
