// PROBE ONLY (#3032), dropped before merge: does React take a DevTools hook installed in
// `vi.hoisted` on CI, and was react-dom already loaded there? Fails on purpose to say so.
import { expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

const seen = vi.hoisted(async () => {
  const { createRequire } = await import("node:module");
  const req = createRequire(import.meta.url);
  const loadedBefore = Object.keys(req.cache).some((k) => /react-dom-client/.test(k));
  const hadHook = typeof (globalThis as Record<string, unknown>).__REACT_DEVTOOLS_GLOBAL_HOOK__;
  const seen = { loadedBefore, hadHook, inject: 0, commits: 0, pid: process.pid };
  (globalThis as Record<string, unknown>).__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    isDisabled: false,
    supportsFiber: true,
    renderers: new Map(),
    inject: () => ++seen.inject,
    checkDCE: () => {},
    onScheduleFiberRoot: () => {},
    onCommitFiberRoot: () => {
      seen.commits += 1;
    },
    onPostCommitFiberRoot: () => {},
    onCommitFiberUnmount: () => {},
  };
  return seen;
});

it("probe: what React sees of a hoisted DevTools hook", async () => {
  render(<div>probe</div>);
  expect(JSON.stringify(await seen)).toBe("see the message");
});
