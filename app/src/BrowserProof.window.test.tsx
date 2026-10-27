// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ShowingProof, ShownProof, TimelineMoment } from "./bindings";
import { BrowserTimeline } from "./BrowserTimeline";

/**
 * **Attach browser proof, from a chat's timeline** (#2026): nothing goes that the person did
 * not pick, the person sees the request, where the pictures are kept and the comment's words
 * before anything is sent, and Attach hands back exactly what was shown.
 */

const painted = vi.hoisted(() => [] as number[][]);
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (_canvas: HTMLCanvasElement, png: Uint8Array) => {
    painted.push(Array.from(png));
  },
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => undefined) }));

const PLANE = "/home/dev/plane";
const CHAT = "01JB0000000000000000000001";
const PR = "https://github.com/acme/api/pull/12";

function moment(over: Partial<TimelineMoment>): TimelineMoment {
  return {
    at: Date.UTC(2026, 9, 26, 9, 0, 0) / 1000,
    browser: "b1",
    by: "agent",
    did: "Took a screenshot",
    page: "https://staging.example.test/cart",
    shot: null,
    unkept: null,
    ...over,
  };
}

const SHOWN: ShownProof = {
  request: PR,
  number: "#12",
  path: "acme/api",
  keptAt: "on the branch purlis/browser-proof/pr-12 of acme/api, in its history for good",
  warning:
    "Public and permanent: anyone who can read acme/api (everyone, on a public repo) sees these, even after the request closes. acme/api is public: everyone can see these.",
  words: "### Browser proof\n\n1 screenshot of what an agent did in a browser.",
  shots: [{ shot: 2, at: 1, digest: "ab".repeat(32) }],
};

/** What show answers: the proof, and the bytes that go ("\x89PN", in base64). */
const SHOWING: ShowingProof = { proof: SHOWN, pictures: ["iVBO"] };

function core(attached: { ok: string } | { refused: string } = { ok: `${PR}#issuecomment-1` }) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "chat_browser_timeline")
      return [moment({ shot: 1 }), moment({ at: Date.UTC(2026, 9, 26, 9, 0, 5) / 1000, shot: 2 })];
    if (cmd === "chat_browser_timeline_shot") return "iVBORw==";
    if (cmd === "browser_proof_requests")
      return [{ url: PR, repo: "api", request: "pull request" }];
    if (cmd === "show_browser_proof") return SHOWING;
    if (cmd === "attach_browser_proof") {
      if ("refused" in attached) throw attached.refused;
      return attached.ok;
    }
    return null;
  });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
  };
}

afterEach(() => {
  cleanup();
  clearMocks();
  painted.length = 0;
});

describe("attach browser proof", () => {
  it("attaches nothing until the person picks a screenshot", async () => {
    const host = core();
    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    const attach = await screen.findByRole("button", { name: "Attach browser proof…" });
    expect(attach).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(attach);

    expect(screen.queryByTestId("browser-proof")).toBeNull();
    expect(host.asked("show_browser_proof")).toEqual([]);
    expect(host.asked("attach_browser_proof")).toEqual([]);
  });

  it("shows exactly what goes, then attaches only what was shown", async () => {
    const host = core();
    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    const boxes = await screen.findAllByRole("checkbox", { name: "Pick for browser proof" });
    // The newest first: the first box is screenshot 2's.
    await userEvent.click(boxes[0]);
    expect(screen.getByText("1 screenshot picked.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Attach browser proof…" }));

    const choose = await screen.findByTestId("browser-proof");
    await waitFor(() =>
      expect(within(choose).getByRole("radio", { name: /The pull request of api/ })).toBeChecked(),
    );
    expect(host.asked("attach_browser_proof")).toEqual([]);
    await userEvent.click(within(choose).getByRole("button", { name: "Show what goes" }));

    const shown = await screen.findByTestId("browser-proof-shown");
    expect(host.asked("show_browser_proof")).toEqual([
      { plane: PLANE, chat: CHAT, request: PR, shots: [2] },
    ]);
    expect(shown).toHaveTextContent("1 screenshot goes to #12 in acme/api, as a comment by you.");
    expect(shown).toHaveTextContent("purlis/browser-proof/pr-12");
    expect(within(shown).getByTestId("browser-proof-warning")).toHaveTextContent(
      "Public and permanent: anyone who can read acme/api",
    );
    expect(within(shown).getByTestId("browser-proof-warning")).toHaveTextContent(
      "acme/api is public",
    );
    // The picture drawn is the bytes show answered, not one read again by its number.
    await waitFor(() => expect(painted).toContainEqual([0x89, 0x50, 0x4e]));
    expect(within(shown).getAllByTestId("browser-proof-picture")).toHaveLength(1);
    expect(within(shown).getByTestId("browser-proof-words")).toHaveTextContent("Browser proof");
    expect(shown).not.toHaveTextContent("staging.example.test");
    expect(host.asked("attach_browser_proof")).toEqual([]);

    await userEvent.click(within(shown).getByRole("button", { name: "Attach proof" }));

    expect(await screen.findByText("The proof is attached.")).toBeInTheDocument();
    expect(host.asked("attach_browser_proof")).toEqual([
      { plane: PLANE, chat: CHAT, shown: SHOWN },
    ]);
  });

  it("says why attaching was refused, and attaches nothing else", async () => {
    const host = core({
      refused: "What you were shown changed since: press Show what goes again before attaching.",
    });
    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);

    const boxes = await screen.findAllByRole("checkbox", { name: "Pick for browser proof" });
    await userEvent.click(boxes[1]);
    await userEvent.click(screen.getByRole("button", { name: "Attach browser proof…" }));
    await waitFor(() =>
      expect(screen.getByRole("radio", { name: /The pull request of api/ })).toBeChecked(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Show what goes" }));
    await userEvent.click(await screen.findByRole("button", { name: "Attach proof" }));

    expect(
      await screen.findByText(
        "What you were shown changed since: press Show what goes again before attaching.",
      ),
    ).toBeInTheDocument();
    expect(host.asked("attach_browser_proof")).toHaveLength(1);
  });

  it("says so when the core could not be asked, and Attach proof can be pressed again", async () => {
    mockIPC((cmd) => {
      if (cmd === "plugin:event|listen") return 1;
      if (cmd === "chat_browser_timeline") return [moment({ shot: 1 })];
      if (cmd === "chat_browser_timeline_shot") return "iVBORw==";
      if (cmd === "browser_proof_requests")
        return [{ url: PR, repo: "api", request: "pull request" }];
      if (cmd === "show_browser_proof") return SHOWING;
      // Not a refusal the core said: the call itself failed.
      if (cmd === "attach_browser_proof") return Promise.reject(new Error("the link went"));
      return null;
    });
    render(<BrowserTimeline plane={PLANE} chat={CHAT} />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "Pick for browser proof" }));
    await userEvent.click(screen.getByRole("button", { name: "Attach browser proof…" }));
    await waitFor(() =>
      expect(screen.getByRole("radio", { name: /The pull request of api/ })).toBeChecked(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Show what goes" }));
    await userEvent.click(await screen.findByRole("button", { name: "Attach proof" }));

    expect(await screen.findByText(/could not be asked/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Attach proof" })).not.toHaveAttribute(
      "aria-disabled",
    );
  });
});
