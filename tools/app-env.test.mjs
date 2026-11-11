// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The environment the relaunch tools give the app they start (#5691): the operator runs them
// from a shell inside a purlis chat, whose own variables must not reach the app under test.
// Run with `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import { appEnv } from "./app-env.mjs";

const A_CHATS_SHELL = {
  PATH: "/usr/bin:/bin",
  HOME: "/Users/someone",
  PURLIS_ROOT: "/the/operators/plane",
  CHARTER_ROOT: "/the/operators/plane",
  PURLIS_CHAT_TOKEN: "the-operators-token",
  CHARTER_CHAT_TOKEN: "the-operators-token",
  PURLIS_HOOK_SOCKET: "/the/operators/hooks.sock",
  CHARTER_HOOK_SOCKET: "/the/operators/hooks.sock",
  PURLIS_PERSONA: "steward",
  CHARTER_WORKSPACE: "smart-ide",
};

test("nothing purlis set in the chat's shell reaches the app", () => {
  const env = appEnv(A_CHATS_SHELL, { PURLIS_ROOT: "/tmp/plane" });
  for (const name of [
    "CHARTER_ROOT",
    "PURLIS_CHAT_TOKEN",
    "CHARTER_CHAT_TOKEN",
    "PURLIS_HOOK_SOCKET",
    "CHARTER_HOOK_SOCKET",
    "PURLIS_PERSONA",
    "CHARTER_WORKSPACE",
  ]) {
    assert.equal(env[name], undefined, `${name} reached the app`);
  }
});

test("the app's plane is the one the tool made, under one name", () => {
  const env = appEnv(A_CHATS_SHELL, { PURLIS_ROOT: "/tmp/plane" });
  assert.equal(env.PURLIS_ROOT, "/tmp/plane");
  assert.equal(env.CHARTER_ROOT, undefined);
});

test("everything else in the shell reaches the app", () => {
  const env = appEnv(A_CHATS_SHELL, {});
  assert.equal(env.PATH, "/usr/bin:/bin");
  assert.equal(env.HOME, "/Users/someone");
});
