import { describe, expect, it } from "vitest";

import { LOCK_AFTER_MS, atLaunch, hidden, shown, unlocked } from "./lock.ts";

describe("the lock", () => {
  it("is locked at launch", () => {
    expect(atLaunch().locked).toBe(true);
  });

  it("stays unlocked when the app comes back before five minutes", () => {
    const away = hidden(unlocked(atLaunch()), 1_000);
    expect(shown(away, 1_000 + LOCK_AFTER_MS - 1).locked).toBe(false);
  });

  it("locks again after five minutes hidden", () => {
    const away = hidden(unlocked(atLaunch()), 1_000);
    expect(shown(away, 1_000 + LOCK_AFTER_MS).locked).toBe(true);
  });

  it("counts from the first time it was hidden", () => {
    const away = hidden(hidden(unlocked(atLaunch()), 1_000), 200_000);
    expect(away.hiddenAt).toBe(1_000);
    expect(shown(away, 1_000 + LOCK_AFTER_MS).locked).toBe(true);
  });

  it("forgets the clock once shown", () => {
    expect(shown(hidden(unlocked(atLaunch()), 1_000), 2_000).hiddenAt).toBeNull();
  });

  it("stays locked when it was locked before it hid", () => {
    expect(shown(hidden(atLaunch(), 0), 1).locked).toBe(true);
  });
});
