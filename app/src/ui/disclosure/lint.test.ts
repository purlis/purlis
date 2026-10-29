// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The disclosure family's lint** (`lint.js` beside this file, #3675): what it refuses and what
 * it lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/disclosure-family", rules["disclosure-family"], {
  valid: [
    { code: `<Disclosure label="More: branch, folder, sign-in">{fields}</Disclosure>` },
    // The one chevron that turns, on a fold that is not a disclosure.
    { code: `<Button aria-expanded={open} icon={<Twist open={open} />}>Finished</Button>` },
    // A chevron that is no fold's, shown or not.
    { code: `<span>{current && <ChevronRight />}</span>` },
    // Two icons that are not chevrons.
    { code: `<span>{busy ? <LoaderCircle /> : <Send />}</span>` },
  ],
  invalid: [
    {
      code: `<button aria-expanded={open}>{open ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}Finished</button>`,
      errors: [{ messageId: "swap" }],
    },
    {
      code: `<MenuItem>{folded ? <ChevronRight /> : <ChevronDown />}</MenuItem>`,
      errors: [{ messageId: "swap" }],
    },
    {
      code: `const icon = shown ? <ChevronUp className="x" /> : <ChevronDown className="x" />;`,
      errors: [{ messageId: "swap" }],
    },
    { code: `<div data-scope="disclosure" data-part="panel" />`, errors: [{ messageId: "scope" }] },
  ],
});
