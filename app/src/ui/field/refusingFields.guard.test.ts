// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a field's error is drawn** (#3254). Only a field that can refuse its value, a
 * `TextField` or a `TextArea` given a `validate` or an `error`, keeps its error's line under its
 * box (`refusable` in `Field.tsx`), and the line's words, in the danger colour, are read on what
 * the field stands on. The theme check holds them at the window's text size on the grounds the
 * parts manifest lists for the error (`FIELD_ERROR_GROUNDS` in
 * `crates/purlis-core/src/theme/parts.rs`), not on every ground a field may stand on.
 *
 * So every file that draws a field that can refuse is listed here with the grounds its field
 * stands on and where, and the manifest's list is exactly theirs and the overlay surface the
 * whole copy of a refusal cut short is drawn on (#3033). A field reached through a wrapper that
 * forwards its refusal (`DRAWS_A_FIELD`) is read where the wrapper is called. A new field that
 * can refuse fails until its file is listed, and a ground it brings until the manifest holds the
 * error's words on it. The grounds are a hand claim: a field that moves to another ground updates
 * its line here.
 */

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { handed, listed, PARTS, SRC, sources } from "./fieldCalls.testkit";

/** Every file that draws a field that can refuse, the grounds that field stands on, and where. */
const REFUSING: Record<string, { grounds: string[]; where: string }> = {
  "AnswerQuestion.tsx": {
    grounds: ["surface.raised", "surface.overlay"],
    where:
      "AnswerBox: under a question's line in the Activity tab's panel, and in the answer dialog (AnswerPanel.tsx)",
  },
  "Panels.tsx": {
    grounds: ["surface.base"],
    where: "AddTodo: at the foot of the todos region panel, the window's own ground",
  },
  "PersonaMarkPicker.tsx": {
    grounds: ["surface.raised"],
    where: "HexColour: in a persona's card, a view tab's panel (Views.tsx)",
  },
  "RaiseLimit.tsx": {
    grounds: ["surface.raised"],
    where:
      "RaiseLimit: a limit's new value, on its own ground, in the Start a chat dialog, under a move's line, in Add a pool, a sign-in's strip and a profile's row on a pool",
  },
  "SkillSources.tsx": {
    grounds: ["surface.base"],
    where: "ClashChoice: a new name in an install's review, a page of the Skills view",
  },
};

/** The ground the whole copy of a refusal cut short is drawn on, wherever its field stands. */
const WHOLE_COPY = "surface.overlay";

/**
 * **The components that draw a field and may hand it a refusal**, by the module that defines
 * them (under `src/`, without its extension): the field family's own two, and each wrapper that
 * forwards a `validate` or an `error` to one (a setting's `Field` and its `ListField`, the
 * palette's `ListComboBox`). A call of any of them given a `validate`, an `error` or a spread of
 * props it cannot read draws a field that can refuse, where the call stands; the forwarding inside
 * a wrapper does not, since its callers decide. A component that forwards a refusal and is not
 * listed here fails (`forwarders`), so a new wrapper cannot hide its callers.
 */
const DRAWS_A_FIELD: Record<string, string[]> = {
  "ui/field/Field": ["TextField", "TextArea"],
  "settings/components": ["Field", "ListField"],
  "ui/select/ComboBox": ["ListComboBox"],
};

/** The props that hand a field a refusal. */
const REFUSALS = ["validate", "error"];

/** What `files` say about refusals: each file that draws a field that can refuse, and each
 *  element that forwards a refusal from a component not listed in `DRAWS_A_FIELD`. */
function read(files: { name: string; text: string }[]): {
  refusing: string[];
  forwarders: string[];
} {
  const { found, forwarders } = handed(files, DRAWS_A_FIELD, REFUSALS, "a refusal");
  return { refusing: found, forwarders };
}

describe("a field that can refuse", () => {
  it("is found where a field is given a validate or an error, under any name it is imported by", () => {
    expect(
      read([
        {
          name: "A.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const A = () => <TextField validate={check} />;`,
        },
        {
          name: "views/B.tsx",
          text: `import { TextArea as Box } from "../ui/field/Field";
            export const B = () => <Box error={said} />;`,
        },
        {
          name: "C.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const C = () => <TextField isInvalid={wrong} />;`,
        },
        {
          name: "D.tsx",
          text: `export const D = () => <TextField validate={check} />;`,
        },
        {
          name: "I.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const I = () => <TextField {...{ [KEY]: "", title: "x" }} />;`,
        },
        {
          name: "J.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export const J = () => <TextField {...{ validate: check }} />;`,
        },
      ]).refusing,
    ).toEqual(["A.tsx", "J.tsx", "views/B.tsx"]);
  });

  it("is found where a wrapper that forwards a refusal is given one, or a spread", () => {
    expect(
      read([
        {
          name: "views/E.tsx",
          text: `import { Field } from "../settings/components";
            export const E = () => <Field kind="text" validate={check} />;`,
        },
        {
          name: "settings/F.tsx",
          text: `import { Field as Row } from "./components";
            export const F = () => <Row kind="text" {...given} />;`,
        },
        {
          name: "G.tsx",
          text: `import * as Pick from "./ui/select/ComboBox";
            export const G = () => <Pick.ListComboBox error={said} />;`,
        },
        {
          name: "H.tsx",
          text: `import { Field } from "./settings/components";
            export const H = () => <Field kind="text" value={v} />;`,
        },
        {
          name: "settings/components.tsx",
          text: `import { TextField } from "../ui/field/Field";
            export function Field(props) { return <TextField validate={props.validate} />; }`,
        },
      ]),
    ).toEqual({ refusing: ["G.tsx", "settings/F.tsx", "views/E.tsx"], forwarders: [] });
  });

  it("fails a wrapper that forwards a refusal and is not listed", () => {
    expect(
      read([
        {
          name: "Mine.tsx",
          text: `import { TextField } from "./ui/field/Field";
            export function Mine(props) { return <TextField validate={props.validate} />; }`,
        },
      ]).forwarders,
    ).toEqual(["Mine.tsx: Mine hands <TextField> a refusal"]);
  });

  it("is never forwarded by a component the list does not name", () => {
    expect(read(sources(SRC)).forwarders).toEqual([]);
  });

  it("is drawn only in the files listed, each with the grounds it stands on", () => {
    expect(read(sources(SRC)).refusing).toEqual(Object.keys(REFUSING).sort());
  });

  it("has its error held on exactly the grounds the parts manifest lists for it", () => {
    const grounds = new Set([WHOLE_COPY, ...Object.values(REFUSING).flatMap((one) => one.grounds)]);
    expect(listed(readFileSync(PARTS, "utf8"), "FIELD_ERROR_GROUNDS")?.sort()).toEqual(
      [...grounds].sort(),
    );
  });
});
