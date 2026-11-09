// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { GalleryPart, PartsManifest } from "../bindings";
import { alsoSaid, familiesSet, fromSaid, partsShown, setSaid, sinceSaid } from "./themeParts";

/** A manifest of two families, as `theme_parts` hands it. */
const MANIFEST: PartsManifest = {
  version: 13,
  states: [
    { name: "hover", description: "The pointer is over it.", since: 1 },
    { name: "selected", description: "It is chosen.", since: 1 },
  ],
  families: [
    {
      name: "button",
      description: "A button, an icon button or a link button.",
      since: 1,
      states: ["hover", "pressed"],
      parts: [
        {
          name: "root",
          description: "The button itself.",
          since: 1,
          properties: ["fill", "corner-radius"],
        },
        { name: "label", description: "Its words.", since: 2, properties: ["text"] },
      ],
      variants: [{ name: "primary", description: "The main answer.", since: 1 }],
    },
    {
      name: "tabs",
      description: "A session tab, docked on the content panel or on a strip of its own.",
      since: 4,
      states: ["hover", "selected"],
      parts: [
        {
          name: "tab",
          description: "One tab.",
          since: 4,
          properties: ["corner-radius", "padding"],
        },
      ],
      variants: [],
    },
  ],
};

const set = (key: string, value: string, from: GalleryPart["from"] = { kind: "theme" }) => {
  const [family, part] = key.split(".");
  return { key, family, part, value, from } satisfies GalleryPart;
};

const ROUNDED: GalleryPart[] = [
  set("button.root.corner-radius", "full", { kind: "option", option: "Pills", value: "On" }),
  set("button.root.primary.hover.fill", "accent.base"),
  set("tabs.tab.corner-radius", "10"),
];

/** What a theme that extends Purlis draws of Purlis's parts. */
const KEPT = set("button.label.text", "text.primary", { kind: "extends", theme: "Purlis" });

describe("a theme's parts, listed from the manifest", () => {
  it("lists every family and part the manifest has, in its order, each with what the theme sets", () => {
    const shown = partsShown(MANIFEST, ROUNDED, "");
    expect(shown.families.map((one) => one.name)).toEqual(["button", "tabs"]);
    const [button, tabs] = shown.families;
    expect(button.parts.map((one) => one.name)).toEqual(["root", "label"]);
    expect(button.parts[0].set.map((one) => one.key)).toEqual([
      "button.root.corner-radius",
      "button.root.primary.hover.fill",
    ]);
    expect(button.parts[1].set).toEqual([]);
    expect(button.set).toBe(2);
    expect(tabs.set).toBe(1);
    expect(shown.parts).toBe(3);
    // Nothing typed: every family is folded, so the tree opens on its families.
    expect(shown.open).toEqual([]);
  });

  it("finds a part by its name, its family's, a description, a key or a value, and opens what it found", () => {
    const radius = partsShown(MANIFEST, ROUNDED, "Corner");
    expect(radius.families.map((one) => [one.name, one.parts.map((part) => part.name)])).toEqual([
      ["button", ["root"]],
      ["tabs", ["tab"]],
    ]);
    expect(radius.open).toEqual(["button", "button.root", "tabs", "tabs.tab"]);
    const words = partsShown(MANIFEST, ROUNDED, "its words");
    expect(words.families.map((one) => one.parts.map((part) => part.name))).toEqual([["label"]]);
    const value = partsShown(MANIFEST, ROUNDED, "accent.base");
    expect(value.families.map((one) => one.parts.map((part) => part.name))).toEqual([["root"]]);
    // A family found by its own name keeps every part.
    const tabs = partsShown(MANIFEST, ROUNDED, "session tab");
    expect(tabs.families.map((one) => one.name)).toEqual(["tabs"]);
    expect(tabs.families[0].parts.map((one) => one.name)).toEqual(["tab"]);
    expect(partsShown(MANIFEST, ROUNDED, "nothing like it").families).toEqual([]);
  });

  it("names the families a theme sets, in the manifest's order, with how many keys", () => {
    expect(familiesSet(MANIFEST, ROUNDED)).toEqual([
      { name: "button", description: "A button, an icon button or a link button.", keys: 2 },
      {
        name: "tabs",
        description: "A session tab, docked on the content panel or on a strip of its own.",
        keys: 1,
      },
    ]);
    expect(familiesSet(MANIFEST, [])).toEqual([]);
  });

  it("counts only the keys a theme or its options set as its own, and says whose the rest are", () => {
    const shown = partsShown(MANIFEST, [...ROUNDED, KEPT], "");
    expect(shown.families[0].set).toBe(2);
    expect(shown.families[0].parts[1].set).toEqual([KEPT]);
    expect(familiesSet(MANIFEST, [KEPT])).toEqual([]);
    expect(setSaid("Rounded", [...ROUNDED, KEPT])).toBe("Rounded sets 3 keys of");
    expect(alsoSaid([...ROUNDED, KEPT])).toBe("It also draws 1 key from Purlis.");
    expect(alsoSaid(ROUNDED)).toBe("");
    expect(setSaid("Plain", [KEPT])).toBe(
      "Plain sets no parts of its own, and draws 1 key from Purlis.",
    );
    expect(setSaid("Bare", [])).toBe(
      "Bare sets no parts: every control is drawn as its family draws it.",
    );
  });

  it("finds a part by a property it takes, and a family by a state it draws", () => {
    const fill = partsShown(MANIFEST, [], "fill");
    expect(fill.families.map((one) => [one.name, one.parts.map((part) => part.name)])).toEqual([
      ["button", ["root"]],
    ]);
    const selected = partsShown(MANIFEST, [], "selected");
    expect(selected.families.map((one) => one.name)).toEqual(["tabs"]);
  });

  it("says what set a key, and the version a part came in", () => {
    expect(fromSaid(KEPT)).toBe("From Purlis");
    expect(fromSaid(ROUNDED[1])).toBe("The theme");
    expect(fromSaid(ROUNDED[0])).toBe("Pills: On");
    expect(sinceSaid(4, MANIFEST)).toBe("Since version 4");
    expect(sinceSaid(14, MANIFEST)).toBe("Next release");
  });
});
