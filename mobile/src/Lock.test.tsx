import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Lock } from "./Lock.tsx";

describe("Lock", () => {
  it("unlocks once the check passes", async () => {
    const onUnlocked = vi.fn();
    render(<Lock onUnlocked={onUnlocked} check={async () => true} />);
    await vi.waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("keeps the lock screen after a failed or cancelled check, with Try again", async () => {
    const onUnlocked = vi.fn();
    const check = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    render(<Lock onUnlocked={onUnlocked} check={check} />);
    const again = await screen.findByRole("button", { name: "Try again" });
    expect(onUnlocked).not.toHaveBeenCalled();
    await userEvent.click(again);
    await vi.waitFor(() => expect(onUnlocked).toHaveBeenCalledTimes(1));
    expect(check).toHaveBeenCalledTimes(2);
  });
});
