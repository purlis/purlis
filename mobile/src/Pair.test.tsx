import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { DesktopRow } from "./ipc.ts";

const { pair } = vi.hoisted(() => ({ pair: vi.fn() }));

vi.mock("./ipc", () => ({
  commands: { pair },
  said: (err: unknown) => (err instanceof Error ? err.message : String(err)),
}));

import { Pair } from "./Pair.tsx";

const mac: DesktopRow = {
  device: "mac",
  name: "Mac",
  addresses: ["10.0.0.2:7777"],
  standing: "connected",
  why: null,
  generation: 1,
};

describe("Pair", () => {
  it("pairs from pasted text, trimmed, and hands over the desktop", async () => {
    pair.mockResolvedValue(mac);
    const onPaired = vi.fn();
    render(<Pair onPaired={onPaired} scan={async () => null} />);
    await userEvent.type(screen.getByLabelText("Pairing text"), "  purlis-pair:abc  ");
    await userEvent.click(screen.getByRole("button", { name: "Pair" }));
    expect(pair).toHaveBeenCalledWith("purlis-pair:abc");
    expect(onPaired).toHaveBeenCalledWith(mac);
  });

  it("asks for the desktop's confirmation while pairing", async () => {
    let settle: (row: DesktopRow) => void = () => {};
    pair.mockReturnValue(new Promise<DesktopRow>((resolve) => (settle = resolve)));
    render(<Pair onPaired={vi.fn()} scan={async () => null} />);
    await userEvent.type(screen.getByLabelText("Pairing text"), "purlis-pair:abc");
    await userEvent.click(screen.getByRole("button", { name: "Pair" }));
    expect(await screen.findByText("Confirm on your desktop…")).toBeInTheDocument();
    settle(mac);
  });

  it("says why when the desktop refused", async () => {
    pair.mockRejectedValue("the desktop refused the pairing: the code has expired");
    render(<Pair onPaired={vi.fn()} scan={async () => null} />);
    await userEvent.type(screen.getByLabelText("Pairing text"), "purlis-pair:abc");
    await userEvent.click(screen.getByRole("button", { name: "Pair" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The desktop refused the pairing: the code has expired",
    );
    expect(screen.getByLabelText("Pairing text")).toBeInTheDocument();
  });

  it("says a bad paste for what it is, not as a refusal", async () => {
    pair.mockRejectedValue("not a purlis pairing code");
    render(<Pair onPaired={vi.fn()} scan={async () => null} />);
    await userEvent.type(screen.getByLabelText("Pairing text"), "hello");
    await userEvent.click(screen.getByRole("button", { name: "Pair" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Not a purlis pairing code");
    expect(screen.getByRole("alert")).not.toHaveTextContent("refused");
  });

  it("pairs from a scanned code", async () => {
    pair.mockResolvedValue(mac);
    const onPaired = vi.fn();
    render(<Pair onPaired={onPaired} scan={async () => "purlis-pair:scanned"} />);
    await userEvent.click(screen.getByRole("button", { name: "Scan" }));
    expect(pair).toHaveBeenCalledWith("purlis-pair:scanned");
    expect(onPaired).toHaveBeenCalledWith(mac);
  });

  it("does nothing when the scan is cancelled", async () => {
    pair.mockClear();
    render(<Pair onPaired={vi.fn()} scan={async () => null} />);
    await userEvent.click(screen.getByRole("button", { name: "Scan" }));
    expect(pair).not.toHaveBeenCalled();
  });
});
