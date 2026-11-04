// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SecretFillNotice } from "./SecretFillNotice";
import { grants } from "./noticeSettle";
import type { FillShown, Shown } from "./bindings";

/**
 * A secret a chat asks purlis to fill is one notice (#5115): the key and its vault by name, the
 * exact site large, the page and the field's label, never a value; Allow once and Keep blocked;
 * and a press names what was drawn.
 */

const fill = (more: Partial<FillShown> = {}): FillShown => ({
  key: "dash-api-key",
  vault: "qa",
  site: "dash.example.test",
  page: "https://dash.example.test/settings",
  label: "API key",
  browser: "b1",
  warnings: [],
  card: false,
  multi_line: false,
  vault_too: false,
  code: false,
  ...more,
});

const ONCE = { id: "once", label: "Allow once", allows: true, preselected: null };
const CHAT = { id: "chat", label: "Allow for this chat", allows: true, preselected: null };
const ALWAYS = { id: "you", label: "Always allow", allows: true, preselected: null };
const KEEP = { id: "keep", label: "Keep blocked", allows: false, preselected: null };

const ask = (more: Partial<FillShown> = {}, options = [ONCE, KEEP]): Shown => ({
  session: 7,
  ask: "secret-fill:1",
  says: "Wants to put dash-api-key from vault qa into a field on dash.example.test",
  options,
  source: "secret-fill",
  chain: [],
  answer: { via: "secret-fill", id: 1, shown: "digest" },
  since: null,
  fill: fill(more),
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("SecretFillNotice", () => {
  it("says in plain words which key goes into which field on which site", () => {
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent(
      "Put dash-api-key from vault qa into the field the page labels API key on",
    );
    expect(screen.getByTestId("fill-site-site")).toHaveTextContent(/^dash\.example\.test$/);
    expect(site).toHaveTextContent("https://dash.example.test/settings");
    expect(site).toHaveTextContent("the chat never sees the value");
  });

  it("says a 2FA code is made from the seed, and that the seed reaches neither page nor chat", () => {
    render(
      <SecretFillNotice plane="p" session={7} asks={[ask({ key: "dash-2fa", code: true })]} />,
    );
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent(
      "Put a 2FA code from dash-2fa in vault qa into the field the page labels API key on",
    );
    expect(site).toHaveTextContent(
      "purlis works the code out from the seed as it fills, and the page and the chat never see the seed.",
    );
    expect(site).not.toHaveTextContent("the chat never sees the value");
  });

  it("offers what the ask offers, and nothing more", () => {
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    expect(screen.getByRole("button", { name: "Allow once" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Keep blocked" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Always/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /this chat/ })).toBeNull();
  });

  it("says each warning, and a field the page labels nothing", () => {
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ label: " ", warnings: ["Vault prod is ops's, not qa's."] })]}
      />,
    );
    expect(screen.getByTestId("fill-site")).toHaveTextContent("into a field on");
    expect(screen.getByTestId("fill-site")).toHaveTextContent("Vault prod is ops's, not qa's.");
  });

  it("shows the line Always allow writes before it is pressed, and sends Always", async () => {
    const line =
      'dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" }';
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_secret_fill") {
        sent(args);
        return { kind: "done", said: "Allowed." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ line, under: "[sandbox.personas.qa] tools" }, [ONCE, CHAT, ALWAYS, KEEP])]}
      />,
    );
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent("Always allow adds this to [sandbox.personas.qa] tools");
    expect(site).toHaveTextContent(line);
    expect(screen.getByRole("button", { name: "Allow for this chat" })).toBeInTheDocument();
    await waitFor(
      async () => {
        await user.click(screen.getByRole("button", { name: "Always allow" }));
        expect(sent).toHaveBeenCalled();
      },
      { timeout: 4000 },
    );
    expect(sent.mock.calls[0]?.[0]).toMatchObject({ press: "always" });
  });

  it("says a card's key is allowed once each time, and shows no line", () => {
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ card: true, line: "visa = {}", under: "[sandbox.personas.qa] tools" })]}
      />,
    );
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent("card or payment value, so it is allowed once each time");
    expect(site).not.toHaveTextContent("Always allow adds");
  });

  it("says why Always allow isn't offered, where it isn't", () => {
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ no_always: "Always allow isn't offered for a chat that runs as no persona" })]}
      />,
    );
    expect(screen.getByTestId("fill-site")).toHaveTextContent(
      "Always allow isn't offered for a chat that runs as no persona.",
    );
  });

  it("says what changed where the notice was drawn again", async () => {
    mockIPC((command) => {
      if (command === "answer_secret_fill")
        return {
          kind: "drawn-again",
          said: "The project's settings for dash-api-key changed. Check the ask, then choose again.",
        };
      return null;
    });
    const user = userEvent.setup();
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    await user.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() =>
      expect(screen.getByText(/settings for dash-api-key changed/)).toBeInTheDocument(),
    );
  });

  it("says a multi-line box is often posted, so it asks each time, and shows no line", () => {
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ multi_line: true, label: "Comment", line: "k = {}", under: "x" })]}
      />,
    );
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent("often posted or published, so you are asked each time");
    expect(site).not.toHaveTextContent("Always allow adds");
  });

  it("draws nothing for another chat", () => {
    render(<SecretFillNotice plane="p" session={8} asks={[ask()]} />);
    expect(screen.queryByTestId("fill-site")).toBeNull();
  });

  it("is a Notice that grants, so a press waits for it to settle", () => {
    expect(grants("secret-fill:secret-fill:1:digest")).toBe(true);
  });

  it("sends the press with the digest of what was drawn", async () => {
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_secret_fill") {
        sent(args);
        return { kind: "done", said: "Allowed once." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    await waitFor(
      async () => {
        await user.click(screen.getByRole("button", { name: "Allow once" }));
        expect(sent).toHaveBeenCalled();
      },
      { timeout: 4000 },
    );
    expect(sent.mock.calls[0]?.[0]).toMatchObject({
      session: 7,
      id: 1,
      press: "once",
      shown: "digest",
      reason: null,
    });
  });

  it("is reached and answered from the keyboard", async () => {
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_secret_fill") {
        sent(args);
        return { kind: "done", said: "Kept blocked. Nothing was filled." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    await waitFor(
      async () => {
        screen.getByRole("button", { name: "Keep blocked" }).focus();
        await user.keyboard("{Enter}");
        expect(sent).toHaveBeenCalled();
      },
      { timeout: 4000 },
    );
    expect(sent.mock.calls[0]?.[0]).toMatchObject({ press: "keep" });
  });
});
