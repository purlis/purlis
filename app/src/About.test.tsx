// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AboutCharter } from "./About";
import { OPEN_LICENCES } from "./LicencesView";

// The third-party notices `vite build` generates (#1784); the dev server and vitest serve none,
// so each test says what the build it stands for would have written.
const thirdParty = vi.hoisted(() => ({
  generated: true,
  packages: [] as {
    kind: "crate" | "npm";
    name: string;
    version: string;
    licence?: string;
    texts: number[];
    standard?: string[];
    authors?: string[];
  }[],
  texts: [] as string[],
}));
vi.mock("virtual:purlis-third-party", () => ({ default: thirdParty }));

afterEach(() => {
  cleanup();
  clearMocks();
  Object.assign(thirdParty, { generated: true, packages: [], texts: [] });
});

describe("About Charter", () => {
  it("opens with the keyboard on Close, as every question does (#1719)", async () => {
    mockIPC(() => new Promise(() => {}));
    render(<AboutCharter />);

    await userEvent.click(screen.getByTestId("title-about"));

    const dialog = await screen.findByRole("dialog");
    await waitFor(() =>
      expect(within(dialog).getByRole("button", { name: "Close" })).toHaveFocus(),
    );
  });

  it("says what it is reading while it reads, in sentence case (#630)", async () => {
    mockIPC(() => new Promise(() => undefined));
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText("Reading what this version brought…")).toBeTruthy();
  });

  it("says why what this version brought could not be read, and Read again asks again", async () => {
    let asks = 0;
    mockIPC((cmd) => {
      if (cmd !== "about_charter") throw new Error("not asked here");
      asks += 1;
      throw new Error("the changelog could not be read");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText(/could not read what this version brought/)).toBeTruthy();

    // NO-8's follow-up (#1296): the dialog reads once, so its refusal offers the retry.
    await userEvent.click(screen.getByRole("button", { name: "Read again" }));
    await waitFor(() => expect(asks).toBe(2));
    expect(await screen.findByText(/could not read what this version brought/)).toBeTruthy();
  });

  it("ships each vendored asset's attribution and licence text (FM-3)", async () => {
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    // No project in front takes the ask for their tab, so they are drawn here (#1719).
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    expect(
      within(notices).getByText(/material-icon-theme 5\.39\.0, under the MIT licence/),
    ).toBeTruthy();
    const texts = [...notices.querySelectorAll("pre")].map((pre) => pre.textContent ?? "");
    expect(texts).toContainEqual(
      expect.stringMatching(/^The MIT License \(MIT\)\nCopyright \(c\) 2025 Material Extensions/),
    );
  });

  it("ships purlis's own NOTICE and Apache License 2.0 first, ahead of what others wrote", async () => {
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    expect(await screen.findByText(/purlis is under the Apache License 2\.0/)).toBeTruthy();
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    const [notice, licence] = [...notices.querySelectorAll("pre")].map(
      (pre) => pre.textContent ?? "",
    );
    expect(notice).toMatch(/^Purlis\nCopyright 2026 Purlis\n/);
    expect(licence).toMatch(/Apache License\s+Version 2\.0, January 2004/);
  });

  it("lists every third-party crate and npm package the build ships, with its licence text, after purlis's own (#1784)", async () => {
    Object.assign(thirdParty, {
      packages: [
        {
          kind: "crate",
          name: "serde",
          version: "1.0.228",
          licence: "MIT OR Apache-2.0",
          texts: [0, 1],
        },
        {
          kind: "crate",
          name: "selectors",
          version: "0.38.0",
          licence: "MPL-2.0",
          texts: [2],
          standard: ["MPL-2.0"],
          authors: ["The Servo Project Developers"],
        },
        { kind: "npm", name: "react", version: "19.2.0", licence: "MIT", texts: [1] },
      ],
      texts: [
        "Apache License text",
        "MIT License\nCopyright (c) Meta",
        "Mozilla Public License Version 2.0",
      ],
    });
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    const third = await within(notices).findByTestId("third-party-notices");
    expect(
      notices.querySelector('[data-testid="own-licence"]')?.compareDocumentPosition(third),
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(within(third).getByText(/2 Rust crates and 1 npm package/)).toBeTruthy();
    const parts = [...third.querySelectorAll("details")].map((part) => [
      part.querySelector("summary")?.textContent,
      part.querySelector("pre")?.textContent,
    ]);
    expect(parts).toEqual([
      ["serde 1.0.228 (MIT OR Apache-2.0)", "Apache License text"],
      ["serde 1.0.228 (MIT OR Apache-2.0), react 19.2.0 (MIT)", "MIT License\nCopyright (c) Meta"],
      [
        "selectors 0.38.0 (MPL-2.0; it ships no licence file, so this is the standard MPL-2.0 text; its authors: The Servo Project Developers)",
        "Mozilla Public License Version 2.0",
      ],
    ]);
  });

  it("says a window that was not built has no third-party notices to list (#1784)", async () => {
    Object.assign(thirdParty, { generated: false });
    mockIPC(() => {
      throw new Error("not asked here");
    });
    render(<AboutCharter />);
    await userEvent.click(screen.getByTestId("title-about"));
    await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
    const notices = await screen.findByTestId("about-notices");
    expect(
      await within(notices).findByText(
        /This window was not built, so it has no third-party notices/,
      ),
    ).toBeTruthy();
  });

  it("opens the licences in a tab of the project in front, and closes (#1719)", async () => {
    mockIPC(() => new Promise(() => {}));
    let asked = 0;
    const take = (event: Event) => {
      asked += 1;
      event.preventDefault();
    };
    window.addEventListener(OPEN_LICENCES, take);
    try {
      render(<AboutCharter />);
      await userEvent.click(screen.getByTestId("title-about"));
      await userEvent.click(await screen.findByRole("link", { name: "their licences" }));
      expect(asked).toBe(1);
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(screen.queryByTestId("about-notices")).toBeNull();
    } finally {
      window.removeEventListener(OPEN_LICENCES, take);
    }
  });
});
