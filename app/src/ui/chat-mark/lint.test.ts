// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The chat mark family's lint** (`lint.js` beside this file, #5340): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/chat-mark-family", rules["chat-mark-family"], {
  valid: [{ code: `<ChatMark state="working" className="pane-persona" />` }],
  invalid: [
    { code: `<span className="state-dot state-running" />`, errors: [{ messageId: "oldClass" }] },
    { code: `<span className="chat-persona" />`, errors: [{ messageId: "oldClass" }] },
    { code: `<span data-scope="chat-mark" />`, errors: [{ messageId: "scope" }] },
  ],
});
