// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState } from "react";
import { commands, type GalleryPart, type PartsManifest } from "../bindings";
import { Badge } from "../ui/badge/Badge";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { ListFilter, noMatchSaid, type Noun } from "../ui/list/ListFilter";
import { RowDescription, RowLabel, RowTrail } from "../ui/list/Row";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { Tree, TreeRow, type Key } from "../ui/tree/Tree";
import type { RowIds } from "./components";

/**
 * **The component parts a theme sets, listed from the parts manifest** (#2947, spec #2897):
 * Settings › Theme's Parts row names the families the chosen theme sets, each with the manifest's
 * own description, and **Show each part** opens every family and part the manifest has, each
 * with its states and the version it came in, and under a part each key the theme sets, with
 * its value and what sets it: the theme, or a value of one of its options.
 *
 * **No part is listed here by hand**: the families, parts, states and descriptions are the
 * manifest's (`theme_parts`, what `purlis theme parts` prints), and the keys and values the
 * engine's (`GalleryCard.parts`). The manifest may hold a thousand entries and more, so the list
 * is a tree of families, folded until a filter finds something in one, drawn only where it is in
 * sight (`drawnInPart`).
 */

/** What the filter calls what it narrows. */
const PARTS: Noun = { one: "part", many: "parts" };

/** One part of a family, with each key the theme sets on it. */
export interface PartShown {
  /** `<family>.<part>`: its row's key in the tree. */
  id: string;
  name: string;
  description: string;
  since: number;
  properties: readonly string[];
  set: readonly GalleryPart[];
}

/** One family, with the parts shown of it and how many keys the theme sets on it. */
export interface FamilyShown {
  name: string;
  description: string;
  since: number;
  states: readonly string[];
  parts: readonly PartShown[];
  set: number;
}

/** What the tree shows: the families, how many parts the manifest has, and which rows are open. */
export interface Shown {
  families: readonly FamilyShown[];
  /** Every part the manifest has: what the filter counts. */
  parts: number;
  /** The rows a filter opens: each family and part it found something in. */
  open: readonly string[];
}

const has = (words: string, typed: string) => words.toLowerCase().includes(typed);

/**
 * **Every family and part of `manifest`**, each with the keys of `set` on it, in the manifest's
 * order; with `query`, only what it finds (a family's name or description keeps all its parts; a
 * part's name, its description, a key or a value keeps that part), opened.
 */
export function partsShown(
  manifest: PartsManifest,
  set: readonly GalleryPart[],
  query: string,
): Shown {
  const typed = query.trim().toLowerCase();
  const open: string[] = [];
  let parts = 0;
  const families: FamilyShown[] = [];
  for (const family of manifest.families) {
    parts += family.parts.length;
    const all = family.parts.map((part) => ({
      id: `${family.name}.${part.name}`,
      name: part.name,
      description: part.description,
      since: part.since,
      properties: part.properties,
      set: set.filter((one) => one.family === family.name && one.part === part.name),
    }));
    const whole =
      typed === "" ||
      has(family.name, typed) ||
      has(family.description, typed) ||
      family.states.some((state) => has(state, typed));
    const kept = whole
      ? all
      : all.filter(
          (part) =>
            has(part.name, typed) ||
            has(part.description, typed) ||
            part.properties.some((property) => has(property, typed)) ||
            part.set.some((one) => has(one.key, typed) || has(one.value, typed)),
        );
    if (kept.length === 0) continue;
    if (typed !== "") open.push(family.name, ...kept.map((part) => part.id));
    families.push({
      name: family.name,
      description: family.description,
      since: family.since,
      states: family.states,
      parts: kept,
      set: all.reduce((count, part) => count + part.set.filter(isOwn).length, 0),
    });
  }
  return { families, parts, open };
}

/** Whether the theme itself sets a key, or one of its options does: not a theme it extends. */
export function isOwn(part: GalleryPart): boolean {
  return part.from.kind !== "extends";
}

/** **The families a theme sets itself**, in the manifest's order, each with how many keys. */
export function familiesSet(
  manifest: PartsManifest,
  set: readonly GalleryPart[],
): { name: string; description: string; keys: number }[] {
  return manifest.families
    .map((family) => ({
      name: family.name,
      description: family.description,
      keys: set.filter((one) => one.family === family.name && isOwn(one)).length,
    }))
    .filter((family) => family.keys > 0);
}

/** What set a key, as its row says it. */
export function fromSaid(part: GalleryPart): string {
  switch (part.from.kind) {
    case "theme":
      return "The theme";
    case "extends":
      return `From ${part.from.theme}`;
    case "option":
      return `${part.from.option}: ${part.from.value}`;
  }
}

/** The manifest version something came in, as a row says it; what no release has shipped yet
 *  comes in the next. */
export function sinceSaid(since: number, manifest: PartsManifest): string {
  return since > manifest.version ? "Next release" : `Since version ${since}`;
}

/** How many part keys, as a sentence counts them. */
export function keysSaid(count: number): string {
  return `${count} ${count === 1 ? "key" : "keys"}`;
}

/** What the themes `set`'s theme extends draw of it, as a sentence's end: "11 keys from
 *  Purlis", or nothing where it extends none that sets parts. */
function keptSaid(set: readonly GalleryPart[]): string {
  const parents = [
    ...new Set(set.flatMap((one) => (one.from.kind === "extends" ? [one.from.theme] : []))),
  ];
  const kept = set.filter((one) => !isOwn(one)).length;
  return kept === 0 ? "" : `${keysSaid(kept)} from ${parents.join(" and ")}`;
}

/** **What the theme `name` sets of `set`, in a sentence's words**: the start of the line its
 *  families follow, where it sets keys of its own or its options do; else the whole line. */
export function setSaid(name: string, set: readonly GalleryPart[]): string {
  const own = set.filter(isOwn).length;
  const kept = keptSaid(set);
  if (own > 0) return `${name} sets ${keysSaid(own)} of`;
  return kept === ""
    ? `${name} sets no parts: every control is drawn as its family draws it.`
    : `${name} sets no parts of its own, and draws ${kept}.`;
}

/** What follows the families a theme sets: what it draws of the themes it extends. */
export function alsoSaid(set: readonly GalleryPart[]): string {
  const kept = keptSaid(set);
  return kept === "" ? "" : `It also draws ${kept}.`;
}

// ---------------------------------------------------------------------------------------
// The manifest, read once
// ---------------------------------------------------------------------------------------

/** The manifest: `undefined` until the core has said, or why it could not. */
type Read = PartsManifest | undefined | { trouble: string };

let manifestRead: Promise<Read> | undefined;

/** **The parts manifest**, asked of the core once for the life of the window: it changes only
 *  with purlis. */
export function useManifest(): Read {
  const [read, setRead] = useState<Read>(undefined);
  useEffect(() => {
    let live = true;
    manifestRead ??= commands
      .themeParts()
      .then((done): Read =>
        done.status === "error"
          ? { trouble: done.error }
          : Array.isArray(done.data?.families)
            ? done.data
            : { trouble: "its answer holds no families" },
      )
      .catch((error: unknown): Read => ({ trouble: String(error) }));
    void manifestRead.then((given) => live && setRead(given));
    return () => {
      live = false;
    };
  }, []);
  return read;
}

/** Forgets the manifest read, for a test that answers it again. */
export function forgetManifest(): void {
  manifestRead = undefined;
}

// ---------------------------------------------------------------------------------------
// The row
// ---------------------------------------------------------------------------------------

/**
 * **The Parts row's control** for the theme `name`, which sets `set`, once the manifest is read:
 * what {@link ThemeParts} draws.
 */
export function ThemePartsRead({
  name,
  set,
  ids,
}: {
  name: string;
  set: readonly GalleryPart[];
  ids: RowIds;
}) {
  const read = useManifest();
  if (read === undefined)
    return (
      <p id={ids.id} className="ui-setting-help ui-text-secondary" role="status">
        Reading the parts manifest…
      </p>
    );
  if ("trouble" in read)
    return (
      <p id={ids.id} className="ui-setting-help ui-text-secondary" role="status">
        purlis could not read the parts manifest: {read.trouble}
      </p>
    );
  return <ThemeParts manifest={read} name={name} set={set} ids={ids} />;
}

/**
 * **What the theme `name`, which sets `set`, sets of the parts of `manifest`**: the families it
 * sets, each named with the manifest's description, and every part under **Show each part**.
 */
export function ThemeParts({
  manifest,
  name,
  set,
  ids,
}: {
  manifest: PartsManifest;
  name: string;
  set: readonly GalleryPart[];
  ids: RowIds;
}) {
  const families = familiesSet(manifest, set);
  return (
    <div
      id={ids.id}
      role="group"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
      className="flex flex-col gap-2"
    >
      {families.length === 0 ? (
        <p className="ui-setting-help ui-text-secondary">{setSaid(name, set)}</p>
      ) : (
        <p className="ui-setting-help ui-text-secondary flex flex-wrap items-center gap-1">
          {setSaid(name, set)}
          {families.map((family) => (
            <Tooltip key={family.name} tip={family.description}>
              <Badge aria-description={`${family.description} ${keysSaid(family.keys)} set.`}>
                {family.name}
              </Badge>
            </Tooltip>
          ))}
        </p>
      )}
      {families.length > 0 && alsoSaid(set) !== "" && (
        <p className="ui-setting-help ui-text-secondary">{alsoSaid(set)}</p>
      )}
      <Disclosure label="Show each part" size="sm">
        <PartsTree manifest={manifest} set={set} />
      </Disclosure>
    </div>
  );
}

/** **Every family and part of the manifest**, under its filter: a tree folded to its families. */
export function PartsTree({
  manifest,
  set,
}: {
  manifest: PartsManifest;
  set: readonly GalleryPart[];
}) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Set<Key>>(new Set());
  // What is open while a filter is typed: what it found, until the person folds or opens a row.
  const [found, setFound] = useState<{ query: string; open: Set<Key> }>();
  const shown = useMemo(() => partsShown(manifest, set, query), [manifest, set, query]);
  const filtering = query.trim() !== "";
  // A filter opens what it found; clearing it puts back what the person had open.
  const expanded = !filtering
    ? open
    : found !== undefined && found.query === query
      ? found.open
      : new Set<Key>(shown.open);
  const id = "theme-parts-tree";
  return (
    <div className="flex flex-col gap-2">
      <ListFilter noun={PARTS} total={shown.parts} query={query} onQuery={setQuery} controls={id} />
      {shown.families.length === 0 ? (
        <p className="ui-setting-help ui-text-secondary" role="status">
          {noMatchSaid(PARTS, query)}
        </p>
      ) : (
        <Tree
          id={id}
          aria-label="Every component part"
          size="sm"
          drawnInPart
          className="max-h-96"
          items={shown.families}
          expandedKeys={expanded}
          onExpandedChange={(keys) =>
            filtering ? setFound({ query, open: new Set(keys) }) : setOpen(new Set(keys))
          }
        >
          {(family) => (
            <TreeRow
              id={family.name}
              textValue={family.name}
              description={family.description}
              content={
                <>
                  <RowLabel className="shrink-0">{family.name}</RowLabel>
                  <RowDescription>
                    {[
                      family.description,
                      family.states.length > 0 ? `States: ${family.states.join(", ")}` : "",
                    ]
                      .filter((one) => one !== "")
                      .join(" ")}
                  </RowDescription>
                  <RowTrail>
                    {family.set > 0 && <Badge variant="accent">{family.set} set</Badge>}{" "}
                    {sinceSaid(family.since, manifest)}
                  </RowTrail>
                </>
              }
            >
              {family.parts.map((part) => (
                <TreeRow
                  key={part.id}
                  id={part.id}
                  textValue={`${family.name}.${part.name}`}
                  description={`${part.description} Sets: ${part.properties.join(", ")}`}
                  content={
                    <>
                      <RowLabel className="shrink-0">{part.name}</RowLabel>
                      <RowDescription>
                        {part.description} Sets: {part.properties.join(", ")}
                      </RowDescription>
                      <RowTrail>
                        {part.set.some(isOwn) && (
                          <Badge variant="accent">{part.set.filter(isOwn).length} set</Badge>
                        )}{" "}
                        {sinceSaid(part.since, manifest)}
                      </RowTrail>
                    </>
                  }
                >
                  {part.set.map((one) => (
                    <TreeRow
                      key={`${one.key}\u0000${fromSaid(one)}`}
                      id={`${one.key}\u0000${fromSaid(one)}`}
                      textValue={one.key}
                      description={`${one.value}, from ${fromSaid(one)}`}
                      content={
                        <>
                          <RowLabel>
                            <code>{one.key.slice(part.id.length + 1)}</code>
                          </RowLabel>
                          <RowDescription>
                            <code>{one.value}</code>
                          </RowDescription>
                          <RowTrail>{fromSaid(one)}</RowTrail>
                        </>
                      }
                    />
                  ))}
                </TreeRow>
              ))}
            </TreeRow>
          )}
        </Tree>
      )}
    </div>
  );
}
