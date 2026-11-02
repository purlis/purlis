// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a field's help is drawn** (#4648). A `TextField` or a `TextArea` given a `help` draws
 * it as the field family's help part (`data-part="help"` in `Field.tsx`), in the quiet words,
 * and the theme check holds those words on the grounds the parts manifest lists for the help
 * (`FIELD_HELP_GROUNDS` in `crates/purlis-core/src/theme/parts.rs`), not on every ground a field
 * may stand on: read on a ground no help is drawn on, a theme's own field colours would be lost
 * for a fault nobody sees.
 *
 * So every file that hands a field its help is listed here with the grounds that field stands on
 * and where, and the manifest's list is exactly theirs. A setting's row draws its own help
 * (`ui-setting-help`, `settings/components.tsx`), not the field's, so it is no help here. A new
 * field given help fails until its file is listed, and a ground it brings until the manifest
 * holds the help's words on it. The grounds are a hand claim: a field that moves to another
 * ground updates its line here.
 */

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { handed, listed, PARTS, SRC, sources } from "./fieldCalls.testkit";

/** Every file that hands a field its help, the grounds that field stands on, and where. */
const HELPED: Record<string, { grounds: string[]; where: string }> = {
  "PoolSetupTab.tsx": {
    grounds: ["surface.raised"],
    where: "Name your pool: a pool's name and its runner image, on a view tab's page (.pane.view)",
  },
  "RaiseLimit.tsx": {
    grounds: ["surface.raised"],
    where: "RaiseLimit: a limit's new value, on its own ground (as refusingFields.guard says)",
  },
  "StartChat.tsx": {
    grounds: ["surface.overlay"],
    where: "the Start a chat dialog: a chat's name, and why it runs without the sandbox",
  },
};

/**
 * **The components that draw a field's help**, by the module that defines them: the field
 * family's own two, and the palette's `ListComboBox`, which hands its props on to one. A setting's
 * `Field` and `ListField` are not among them: their help is the row's.
 */
const DRAWS_HELP: Record<string, string[]> = {
  "ui/field/Field": ["TextField", "TextArea"],
  "ui/select/ComboBox": ["ListComboBox"],
};

/** What `files` say about help: each file that hands a field its help, and each element that
 *  forwards it from a component not listed in `DRAWS_HELP`. */
function read(files: { name: string; text: string }[]) {
  return handed(files, DRAWS_HELP, ["help"], "its help");
}

describe("a field's help", () => {
  it("is found where a field is given help, under any name, or a spread that may carry it", () => {
    expect(
      read([
        {
          name: "A.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const A = () => <TextField help="Small letters." />;`,
        },
        {
          name: "views/B.tsx",
          text: `import { TextArea as Box } from "../ui/field/Field";
            export const B = () => <Box {...given} />;`,
        },
        {
          name: "C.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const C = () => <TextField {...(said ? { error: said } : {})} />;`,
        },
        {
          name: "D.tsx",
          text: `import { Field } from "./settings/components";
            export const D = () => <Field kind="text" help="A setting's own." />;`,
        },
        {
          name: "E.tsx",
          text: `import * as Pick from "./ui/select/ComboBox";
            export const E = () => <Pick.ListComboBox help={said} />;`,
        },
      ]),
    ).toEqual({ found: ["A.tsx", "E.tsx", "views/B.tsx"], forwarders: [] });
  });

  it("fails a wrapper that forwards help and is not listed", () => {
    expect(
      read([
        {
          name: "Mine.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export function Mine(props) { return <TextField help={props.help} />; }`,
        },
      ]).forwarders,
    ).toEqual(["Mine.tsx: Mine hands <TextField> its help"]);
  });

  it("is never forwarded by a component the list does not name", () => {
    expect(read(sources(SRC)).forwarders).toEqual([]);
  });

  it("is drawn only in the files listed, each with the grounds it stands on", () => {
    expect(read(sources(SRC)).found).toEqual(Object.keys(HELPED).sort());
  });

  it("is held on exactly the grounds the parts manifest lists for it", () => {
    const grounds = new Set(Object.values(HELPED).flatMap((one) => one.grounds));
    expect(listed(readFileSync(PARTS, "utf8"), "FIELD_HELP_GROUNDS")?.sort()).toEqual(
      [...grounds].sort(),
    );
  });
});
