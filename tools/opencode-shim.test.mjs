// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// purlis's opencode plugin (`app/src-tauri/plugin/opencode/purlis.ts`, generated from
// `purlis_core::opencode`), run as opencode runs it: its `tool.execute.before` is handed a tool
// call, and a throw refuses the call with the thrown message as the tool's error (measured on
// opencode 1.18.33, #4559). `Bun.spawn` stands in for `purlis hook <word>`. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

const SHIM = new URL(
  "../app/src-tauri/plugin/opencode/purlis.ts",
  import.meta.url,
);

// The plugin's hooks, with `purlis hook <word>` answering as `answer(word, payload)` says:
// `{ code, out, err }`, or `"hangs"` for a program that never answers.
async function plugin(answer) {
  const ran = [];
  globalThis.Bun = {
    spawn: ([, , word], options) => {
      const payload = options.stdin.text().then((text) => JSON.parse(text));
      const said = payload.then((payload) => {
        ran.push({ word, payload });
        return answer(word, payload);
      });
      const hangs = new Promise(() => {});
      const field = (key) =>
        said.then((it) => (it === "hangs" ? hangs : it[key]));
      return {
        stdout: new ReadableStream({
          async start(controller) {
            controller.enqueue(new TextEncoder().encode(await field("out")));
            controller.close();
          },
        }),
        stderr: new ReadableStream({
          async start(controller) {
            controller.enqueue(new TextEncoder().encode(await field("err")));
            controller.close();
          },
        }),
        exited: field("code"),
        kill: () => {},
      };
    },
  };
  process.env.CHARTER_HOOK_BINARY = "/bin/purlis";
  // A fresh copy each time: the plugin takes `Bun.spawn` and the binary when it loads.
  const { PurlisPlugin } = await import(`${SHIM.href}?${Math.random()}`);
  const hooks = await PurlisPlugin({ directory: "/work", client: {} }, {});
  return { hooks, ran };
}

// opencode's own question, as its `tool.execute.before` is handed it (1.18.33).
const QUESTION = [
  { tool: "question", sessionID: "ses_task", callID: "call_1" },
  {
    args: {
      questions: [
        {
          question: "Which colour?",
          header: "Which colour?",
          options: [
            { label: "Red", description: "Choose red." },
            { label: "Blue", description: "Choose blue." },
          ],
        },
      ],
    },
  },
];

const deny = (reason) => ({
  code: 0,
  out: JSON.stringify({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: reason,
    },
  }),
  err: "",
});

test("a task's question is handed to purlis and refused with where it went", async () => {
  const { hooks, ran } = await plugin(() =>
    deny("Your asking chat has your question."),
  );

  await assert.rejects(hooks["tool.execute.before"](...QUESTION), {
    message: "Your asking chat has your question.",
  });
  assert.deepEqual(ran, [
    {
      word: "taskquestion",
      payload: {
        hook_event_name: "PreToolUse",
        session_id: "ses_task",
        cwd: "/work",
        tool_name: "question",
        tool_input: QUESTION[1].args,
      },
    },
  ]);
});

test("a question purlis leaves to the person is asked in opencode's pane", async () => {
  const { hooks } = await plugin(() => ({ code: 0, out: "", err: "" }));

  await hooks["tool.execute.before"](...QUESTION);
});

test("a question is never dropped when purlis cannot answer", async () => {
  for (const said of [
    { code: 1, out: "", err: "crashed" },
    { code: 2, out: "", err: "a guard's refusal" },
    { code: 0, out: "not json", err: "" },
    {
      code: 0,
      out: JSON.stringify({
        hookSpecificOutput: { permissionDecision: "deny" },
      }),
      err: "",
    },
  ]) {
    const { hooks } = await plugin(() => said);
    await hooks["tool.execute.before"](...QUESTION);
  }
});

test(
  "a question purlis does not answer in time is asked in opencode's pane",
  { timeout: 20000 },
  async () => {
    const { hooks } = await plugin(() => "hangs");

    await hooks["tool.execute.before"](...QUESTION);
  },
);

test("any other tool still goes to its guard, which refuses when it cannot answer", async () => {
  const { hooks, ran } = await plugin(() => ({
    code: 1,
    out: "",
    err: "crashed",
  }));

  await assert.rejects(
    hooks["tool.execute.before"](
      { tool: "bash", sessionID: "ses_task" },
      { args: { command: "ls" } },
    ),
    /could not answer/,
  );
  assert.equal(ran[0].word, "pretooluse");
});
