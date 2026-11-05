// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **A Secret ask's fill** (#5068, #5073): a new value, or an entry the person keeps. */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import type { SecretShown } from "./bindings";
import { SecretFill } from "./SecretFill";
import type { VaultEntries, VaultEntry } from "./VaultEntryPicker";
import { selectNamed } from "./ui/select/select.testkit";

afterEach(cleanup);

const secret = {
  persona: "devops",
  vault: "team",
  key: "KUBECONFIG",
  tool: null,
  waiting: 1,
  declared: true,
};

const KEPT: VaultEntries[] = [
  { vault: "own", keys: ["STRIPE"] },
  { vault: "team", keys: ["STRIPE"] },
];

function Held({ onPicked }: { onPicked: (entry: VaultEntry | undefined) => void }) {
  const [picked, setPicked] = useState<VaultEntry>();
  return (
    <SecretFill
      secret={secret}
      box={null}
      entries={() => Promise.resolve(KEPT)}
      picked={picked}
      onPicked={(entry) => {
        setPicked(entry);
        onPicked(entry);
      }}
    />
  );
}

/** The fill of a key its persona does not declare, holding Remember as the Inbox row does. */
function Remembering({
  shown,
  onRemember,
}: {
  shown: NonNullable<SecretShown["remember"]>;
  onRemember: (purpose: string | undefined) => void;
}) {
  const [remember, setRemember] = useState<string>();
  return (
    <SecretFill
      secret={{ ...secret, declared: false, remember: shown }}
      box={null}
      entries={() => Promise.resolve(KEPT)}
      picked={undefined}
      onPicked={() => {}}
      remember={remember}
      onRemember={(to) => {
        setRemember(to);
        onRemember(to);
      }}
    />
  );
}

describe("remembering a need on a fill (#5240)", () => {
  const shown = {
    file: "personas/devops/persona.md",
    need: true,
    purpose: "the chat's words",
    vault: "team",
  };

  it("is unticked, and shows no line until it is ticked", () => {
    render(<Remembering shown={shown} onRemember={() => {}} />);
    expect(
      screen.getByRole("checkbox", { name: "Remember this need for the persona" }),
    ).not.toBeChecked();
    expect(screen.queryByText("secret-need.KUBECONFIG:")).toBeNull();
    expect(screen.queryByText(/personas\/devops\/persona\.md/)).toBeNull();
  });

  it("shows the exact line once ticked, with the chat's words to edit and the vault line", async () => {
    const user = userEvent.setup();
    const said = vi.fn();
    render(<Remembering shown={shown} onRemember={said} />);
    await user.click(screen.getByRole("checkbox", { name: "Remember this need for the persona" }));

    expect(said).toHaveBeenLastCalledWith("the chat's words");
    expect(screen.getByText("secret-need.KUBECONFIG:")).toBeInTheDocument();
    expect(screen.getByText("vault: team")).toBeInTheDocument();
    expect(screen.getByText(/personas\/devops\/persona\.md/)).toBeInTheDocument();
    const purpose = screen.getByRole("textbox", { name: "What KUBECONFIG is for" });
    expect(purpose).toHaveValue("the chat's words");

    await user.clear(purpose);
    await user.type(purpose, "reach staging");
    expect(said).toHaveBeenLastCalledWith("reach staging");
  });

  it("says nothing goes in once it is unticked again", async () => {
    const user = userEvent.setup();
    const said = vi.fn();
    render(<Remembering shown={shown} onRemember={said} />);
    const box = screen.getByRole("checkbox", { name: "Remember this need for the persona" });
    await user.click(box);
    await user.click(box);
    expect(said).toHaveBeenLastCalledWith(undefined);
    expect(screen.queryByText("secret-need.KUBECONFIG:")).toBeNull();
  });

  it("shows only the vault line for a key the persona declares", async () => {
    const user = userEvent.setup();
    render(<Remembering shown={{ ...shown, need: false }} onRemember={() => {}} />);
    await user.click(screen.getByRole("checkbox", { name: "Remember this need for the persona" }));
    expect(screen.getByText("vault: team")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "What KUBECONFIG is for" })).toBeNull();
  });

  it("offers nothing where nothing would be written", () => {
    render(
      <SecretFill
        secret={secret}
        box={null}
        entries={() => Promise.resolve(KEPT)}
        picked={undefined}
        onPicked={() => {}}
      />,
    );
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});

describe("a secret ask's fill", () => {
  it("picks an entry by its vault and key, told apart where two vaults hold one name", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Held onPicked={picked} />);
    await user.click(screen.getByRole("radio", { name: "Use an entry I have" }));
    const picker = selectNamed("Entry KUBECONFIG reads");
    act(() => picker.focus());
    await user.keyboard("{Enter}");
    const list = await screen.findByRole("listbox");
    const groups = within(list).getAllByRole("group");
    await user.click(within(groups[1]).getByRole("option", { name: "STRIPE" }));
    expect(picked).toHaveBeenLastCalledWith({ vault: "team", key: "STRIPE" });
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent(
        "KUBECONFIG becomes a link to STRIPE in team",
      ),
    );
  });

  it("forgets the pick as the person goes back to a new value, so Fill sends the value", async () => {
    const user = userEvent.setup();
    const picked = vi.fn();
    render(<Held onPicked={picked} />);
    await user.click(screen.getByRole("radio", { name: "Use an entry I have" }));
    await user.click(screen.getByRole("radio", { name: "New value" }));
    expect(picked).toHaveBeenLastCalledWith(undefined);
    expect(screen.getByLabelText("Value of KUBECONFIG")).toHaveAttribute("type", "password");
  });
});

describe("a secret ask that replaces or mends (#5075)", () => {
  const entries = () => Promise.resolve(KEPT);
  it("says the chat's reason as the chat's, and names the entry a link reads", () => {
    render(
      <SecretFill
        secret={{
          ...secret,
          replace: "401 from the API",
          reads: { vault: "own", key: "STRIPE" },
        }}
        box={null}
        entries={entries}
        picked={undefined}
        onPicked={() => {}}
      />,
    );
    expect(screen.getByTestId("secret-replace")).toHaveTextContent(
      "The chat says the current value doesn't work: 401 from the API",
    );
    expect(screen.getByText(/A new value goes there/)).toHaveTextContent(
      "KUBECONFIG reads STRIPE in vault own",
    );
  });

  it("names the entry a broken link read, which is gone", () => {
    render(
      <SecretFill
        secret={{ ...secret, gone: { vault: "own", key: "STRIPE" } }}
        box={null}
        entries={entries}
        picked={undefined}
        onPicked={() => {}}
      />,
    );
    expect(screen.getByTestId("secret-gone")).toHaveTextContent(
      "KUBECONFIG was linked to STRIPE in vault own, which is gone",
    );
  });
});
