import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Without `globals`, testing-library does not register its own cleanup: each test's render
// would stay in the document for the next one to find twice.
afterEach(cleanup);

// jsdom has no ResizeObserver; the terminal's holder watches its own size with one.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// jsdom has no `matchMedia`; the theme follows the system's appearance through it.
globalThis.matchMedia ??= ((query: string) =>
  ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent: () => false,
  }) as MediaQueryList) as typeof matchMedia;
