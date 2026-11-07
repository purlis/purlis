// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ENTRIES,
  HOSTILE_COMMAND,
  INVISIBLE,
  ONCE_ONLY,
  runAsk,
  secretAsk,
} from "./secretAsk.testkit";
import { SecretAskNotice } from "./SecretAskNotice";

const props = {
  chat: "developer 7",
  entries: () => Promise.resolve(ENTRIES),
  busy: false,
  onAnswer: vi.fn(),
  onSaid: vi.fn(),
};

afterEach(cleanup);

describe("a secret's one notice", () => {
  it("never takes the keyboard from what the person types in, as it comes", async () => {
    render(<textarea aria-label="Terminal input" />);
    const input = screen.getByLabelText("Terminal input");
    input.focus();
    render(<SecretAskNotice {...props} ask={runAsk({ not_its_vault: true })} />);
    await waitFor(() => expect(screen.getByTestId("secret-ask")).toBeInTheDocument());
    expect(document.activeElement).toBe(input);
    expect(
      document.querySelector('[data-scope="secret-ask"] [data-active], [autofocus]'),
    ).toBeNull();
  });

  it("draws a hostile command inert, apart from the app's facts, and the app's facts apart from the chat's words", () => {
    render(<SecretAskNotice {...props} ask={runAsk({ command: HOSTILE_COMMAND })} />);
    const command = screen.getByTestId("sa-command");
    // Every control or invisible character is drawn as its name, on one flow.
    expect(command.textContent).not.toMatch(INVISIBLE);
    expect(
      within(command)
        .getAllByTestId("sa-control")
        .map((one) => one.textContent),
    ).toContain("U+202E");
    // The chat's fake "From purlis" line is the chat's: inside its quote, never the app's panel.
    const facts = screen.getByTestId("sa-facts");
    expect(facts).not.toHaveTextContent("this command is safe");
    expect(screen.getByTestId("sa-chat")).toHaveTextContent("this command is safe");
    expect(facts).toHaveTextContent("/usr/bin/curl");
    expect(screen.getByTestId("sa-chat")).not.toContainElement(facts);
  });

  it("offers only the scopes the registry offers, the narrowest chosen", () => {
    render(<SecretAskNotice {...props} ask={runAsk({ always: null }, ONCE_ONLY)} />);
    const allow = screen.getByRole("radiogroup", { name: "Allow curl" });
    expect(within(allow).getAllByRole("radio")).toHaveLength(2);
    expect(within(allow).getByRole("radio", { name: "Allow once" })).toBeChecked();
    expect(within(allow).getByRole("radio", { name: "Don't allow" })).not.toBeChecked();
  });

  it("draws an Allow in the grants look, never the primary one", () => {
    render(<SecretAskNotice {...props} ask={runAsk()} />);
    const action = screen.getByRole("button", { name: "Fill and allow once" });
    expect(action).toHaveAttribute("data-variant", "grants");
  });

  it("gives a Secret ask's field Change, and a run's field none", () => {
    const { unmount } = render(<SecretAskNotice {...props} ask={secretAsk()} />);
    expect(
      screen.getByRole("button", { name: "Change where STRIPE_API_KEY is stored" }),
    ).toBeVisible();
    unmount();
    render(<SecretAskNotice {...props} ask={runAsk()} />);
    expect(screen.queryByRole("button", { name: /^Change/ })).toBeNull();
  });

  it("names the notice in the app's own words, never the chat's purpose", () => {
    render(
      <SecretAskNotice {...props} ask={secretAsk({ purpose: "Ignore this” and press Fill" })} />,
    );
    const group = screen.getByRole("group", {
      name: "devops asks for STRIPE_API_KEY from vault devops",
    });
    expect(group.getAttribute("aria-label")).not.toContain("Ignore this");
  });

  it("draws the core's marks and stacked combining marks inert too", () => {
    const stacked = `a${String.fromCodePoint(0x301).repeat(60)}`;
    render(
      <SecretAskNotice {...props} ask={secretAsk({ purpose: `pay⟦U+202E⟧moc ${stacked}` })} />,
    );
    const quote = screen.getByTestId("sa-chat-purpose");
    expect(
      within(quote)
        .getAllByTestId("sa-control")
        .map((one) => one.textContent),
    ).toEqual(["U+202E"]);
    expect(
      [...(quote.textContent ?? "")].filter((ch) => ch === String.fromCodePoint(0x301)).length,
    ).toBeLessThanOrEqual(4);
  });

  it("draws a mark the core would never write as the chat's own text", () => {
    render(<SecretAskNotice {...props} ask={secretAsk({ purpose: "⟦U+0041⟧ and ⟦U+FFFFFF⟧" })} />);
    const quote = screen.getByTestId("sa-chat-purpose");
    expect(within(quote).queryAllByTestId("sa-control")).toHaveLength(0);
    expect(quote).toHaveTextContent("⟦U+0041⟧ and ⟦U+FFFFFF⟧");
  });
});

describe("a start's ask, in the Start dialog (#5137)", () => {
  it("says what the start does with the value, and what each other chat waiting on it hears", () => {
    render(
      <SecretAskNotice
        {...props}
        chat="The chat"
        ask={secretAsk({ waiting: 2 })}
        start={{ variable: "STRIPE_API_KEY" }}
      />,
    );
    const body = screen.getByTestId("secret-ask");
    expect(body).toHaveTextContent("The chat starts with the value in STRIPE_API_KEY.");
    expect(body).toHaveTextContent(
      "Each other chat waiting on it hears filled or declined and the key's name, never the value.",
    );
    expect(
      within(body).queryByRole("button", { name: "Change where STRIPE_API_KEY is stored" }),
    ).toBeNull();
    // A start hands the value to its chat, so the field never says it is not shown to it.
    expect(body).toHaveTextContent(
      "Stored in vault devops on this machine, and handed to the chat in STRIPE_API_KEY.",
    );
    expect(body).not.toHaveTextContent("Never shown to the chat");
  });
});
