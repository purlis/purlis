// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { asDrawn, asNamed } from "./projectExpects";

/**
 * **What an answer to a project's expects is sent** (#1888, #1927), from the consent's row in
 * Asks (#6000, R-205): each item as it was drawn, so the backend keeps only what the person saw.
 */

const DOCKER = {
  what: "socket",
  target: "/opt/tools/harmless.sock",
  named: "/Users/op/.docker/run/docker.sock",
  says: "Allowing it lets this chat control containers.",
  refused: null,
} as const;
const GONE = {
  ...DOCKER,
  target: "/opt/project/run/dev.sock",
  named: "/opt/project/run/dev.sock",
  refused: "That is not a local socket on this machine now, so there is nothing to allow.",
} as const;

describe("what an answer to a project's expects is sent", () => {
  it("sends an Allow each item by what the file wrote, with the name it was drawn under", () => {
    expect(asDrawn([DOCKER])).toEqual([
      { target: "/opt/tools/harmless.sock", named: "/Users/op/.docker/run/docker.sock" },
    ]);
  });

  it("sends an item drawn as one that cannot be allowed with no name, so an Allow never keeps it", () => {
    expect(asDrawn([GONE, DOCKER])).toEqual([
      { target: "/opt/project/run/dev.sock", named: null },
      { target: "/opt/tools/harmless.sock", named: "/Users/op/.docker/run/docker.sock" },
    ]);
  });

  it("sends Not now each item with the name it was shown under, one that cannot be allowed too", () => {
    expect(asNamed([GONE, DOCKER])).toEqual([
      { target: "/opt/project/run/dev.sock", named: "/opt/project/run/dev.sock" },
      { target: "/opt/tools/harmless.sock", named: "/Users/op/.docker/run/docker.sock" },
    ]);
  });
});
