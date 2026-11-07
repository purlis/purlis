// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { appProcessesIn, descriptorsListed } from "./processes.js";

/**
 * How a macOS run counts the app's open descriptors: `lsof -F f`, whose `f` field is either a
 * descriptor number or a name for something that is not one (the working directory, the
 * program's text, a mapped library). Only the numbers are descriptors (SC-15).
 */
describe("the descriptors lsof lists", () => {
  it("are its numbered f fields, and not the cwd, txt or mapped entries", () => {
    const listed = ["p4242", "fcwd", "ftxt", "ftxt", "f0", "f1", "f2", "f17", "f256", ""].join(
      "\n",
    );
    expect(descriptorsListed(listed)).toBe(5);
  });

  it("are none when lsof listed nothing for the process", () => {
    expect(descriptorsListed("")).toBe(0);
  });
});

/**
 * How a run tells the app's window from the session host it starts (#2085, #2384). The host is
 * the same binary, started with `purlis-serve-for-the-window` as its first argument, so a count
 * by the binary's name alone found two apps and the stress run stopped before measuring.
 */
describe("the app's processes in a ps listing", () => {
  const app = "/runner/target/debug/purlis-app";

  it("are one window and its session host, told apart by the host's argument", () => {
    const listing = [
      "  101 /bin/zsh -l",
      `23721 ${app}`,
      `23778 ${app} purlis-serve-for-the-window`,
      "",
    ].join("\n");
    expect(appProcessesIn(listing, app)).toEqual({ windows: [23721], sessionHosts: [23778] });
  });

  it("are two windows when two apps really run, host or not", () => {
    const listing = [
      `23721 ${app}`,
      `23778 ${app} purlis-serve-for-the-window`,
      `24000 ${app}`,
    ].join("\n");
    expect(appProcessesIn(listing, app).windows).toEqual([23721, 24000]);
  });

  it("are never the binary started as a helper: a chat's sandbox wrap, a keeper, a reader (#5586)", () => {
    const listing = [
      `23721 ${app}`,
      `23778 ${app} purlis-serve-for-the-window`,
      `24000 ${app} _sandbox --profile /tmp/p -- /bin/fake-harness`,
      `24001 ${app} _no-namespaces -- /bin/fake-harness`,
      `24002 ${app} purlis-engine-keeper 7`,
      `24003 ${app} purlis-git-credential get`,
      `24004 ${app} charter-hold-a-keyring-item`,
      `24005 ${app} /repo main charter-read-a-branch`,
    ].join("\n");
    expect(appProcessesIn(listing, app)).toEqual({ windows: [23721], sessionHosts: [23778] });
  });

  it("are not another binary whose path only begins with the app's", () => {
    const listing = [`300 ${app}-helper`, `301 /usr/bin/grep ${app}`].join("\n");
    expect(appProcessesIn(listing, app)).toEqual({ windows: [], sessionHosts: [] });
  });
});
