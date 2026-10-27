// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family's lint** (`lint.js` beside this file, #2904): what it refuses and what it
 * lets through, on code a screen would really write.
 */
import { rules, tester } from "../lint.testkit";

tester.run("purlis/table-family", rules["table-family"], {
  valid: [
    {
      code: `<Table aria-label="Hosts" rowMenus><TableHeader><TableColumn isRowHeader>Host</TableColumn></TableHeader><TableBody>{rows}</TableBody></Table>`,
    },
    { code: `<ControlTable aria-label="Dispatch limits" stacks>{rows}</ControlTable>` },
    { code: `<ControlCell heading="Depth" className="dispatch-depth">{box}</ControlCell>` },
    // A list's grid is the list family's, and a selector that reads a row draws none.
    { code: `table.querySelector('[role="row"]')` },
    { code: `<div role="group" aria-label="Hosts">x</div>` },
  ],
  invalid: [
    { code: `<table className="secrets">{rows}</table>`, errors: [{ messageId: "element" }] },
    {
      code: `<tr><td>{a}</td></tr>`,
      errors: [{ messageId: "element" }, { messageId: "element" }],
    },
    { code: `<th scope="col">Name</th>`, errors: [{ messageId: "element" }] },
    { code: `<div role="grid" aria-label="Secrets">{rows}</div>`, errors: [{ messageId: "role" }] },
    { code: `<div role={"row"}>x</div>`, errors: [{ messageId: "role" }] },
    { code: `<span role={on ? "gridcell" : "none"}>x</span>`, errors: [{ messageId: "role" }] },
    {
      code: `<div className="ui-table-room">{table}</div>`,
      errors: [{ messageId: "oldClass" }],
    },
    { code: `<span className="ui-num">3</span>`, errors: [{ messageId: "oldClass" }] },
    { code: `<div data-scope="table">x</div>`, errors: [{ messageId: "scope" }] },
  ],
});
