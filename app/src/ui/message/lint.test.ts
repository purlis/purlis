// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The message family's lint** (`lint.js` beside this file, #5649). */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/message-family", rules["message-family"], {
  valid: [
    { code: `<MessageRow tone="waiting" icon={mark}><MessageTop title={who} /></MessageRow>` },
    { code: `<li className="inbox-recent">{said}</li>` },
  ],
  invalid: [
    {
      code: `<li data-scope="message" data-part="root">x</li>`,
      errors: [{ messageId: "scope" }],
    },
  ],
});
