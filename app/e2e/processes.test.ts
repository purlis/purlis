// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { parseProcesses } from "./memory.js";
import { appProcessesIn, appProcessesOf, descriptorsListed, theRunsApp } from "./processes.js";

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

/**
 * The app this run's launcher started, and the session hosts that app started (#5653): what
 * the upgrade run ends before it removes its temporary plane. By the process tree, so another
 * run's app on the same build is never touched.
 */
describe("the run's app in a ps listing with parents", () => {
  const app = "/runner/target/debug/purlis-app";
  const launcher = 5000;
  /** `ps -A -o pid=,ppid=,rss=,etime=,command=`, as `processes()` reads it. */
  const rows = (lines: string[]) => parseProcesses(lines.join("\n"), "");

  it("is the window the launcher started and the session host that window started", () => {
    const listing = [
      "   1 0 1 00:01 /sbin/launchd",
      " 5000   900 1 00:01 node wdio run e2e/wdio.upgrade.conf.ts",
      ` 5100 5000 1 00:01 ${app}`,
      ` 5200 5100 1 00:01 ${app} purlis-serve-for-the-window`,
      "",
    ];
    expect(theRunsApp(rows(listing), launcher, app)).toEqual({
      windows: [5100],
      sessionHosts: [5200],
      programs: [],
    });
  });

  it("is found below a driver the launcher started, as on Linux", () => {
    const listing = [
      "5050 5000 1 00:01 /usr/bin/WebKitWebDriver --port=4445",
      ` 5100 5050 1 00:01 ${app}`,
      ` 5200 5100 1 00:01 ${app} purlis-serve-for-the-window`,
    ];
    expect(theRunsApp(rows(listing), launcher, app)).toEqual({
      windows: [5100],
      sessionHosts: [5200],
      programs: [],
    });
  });

  it("is not another run's app on the same build, nor its host", () => {
    const listing = [
      ` 6100 6000 1 00:01 ${app}`,
      ` 6200 6100 1 00:01 ${app} purlis-serve-for-the-window`,
      "5300 5000 1 00:01 /usr/bin/true",
    ];
    expect(theRunsApp(rows(listing), launcher, app)).toEqual({
      windows: [],
      sessionHosts: [],
      programs: [],
    });
  });

  it("is one window when the window started its crash handler's companion, as on macOS (#5691)", () => {
    const listing = [
      ` 5100 5000 1 00:01 ${app}`,
      ` 5105 5100 1 00:01 ${app}`,
      ` 5200 5100 1 00:01 ${app} purlis-serve-for-the-window`,
    ];
    expect(theRunsApp(rows(listing), launcher, app)).toEqual({
      windows: [5100],
      sessionHosts: [5200],
      programs: [5105],
    });
    expect(appProcessesOf(rows(listing), app)).toEqual({ windows: [5100], sessionHosts: [5200] });
  });

  it("is still two windows when two apps really run, each with its companion", () => {
    const listing = [
      ` 5100 5000 1 00:01 ${app}`,
      ` 5105 5100 1 00:01 ${app}`,
      ` 6100 5000 1 00:01 ${app}`,
      ` 6105 6100 1 00:01 ${app}`,
    ];
    expect(appProcessesOf(rows(listing), app).windows).toEqual([5100, 6100]);
  });

  it("holds the programs its host started, each in a group of its own, to end after the host", () => {
    const listing = [
      ` 5100 5000 1 00:01 ${app}`,
      ` 5200 5100 1 00:01 ${app} purlis-serve-for-the-window`,
      " 5300 5200 1 00:01 /runner/target/debug/fake-harness --resume",
      " 5400 5300 1 00:01 /bin/sh -c sleep 60",
    ];
    expect(theRunsApp(rows(listing), launcher, app)).toEqual({
      windows: [5100],
      sessionHosts: [5200],
      programs: [5300, 5400],
    });
  });
});
