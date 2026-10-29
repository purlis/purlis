// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { NeedKind, NeedsYou } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { Badge } from "./ui/badge/Badge";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow, ListRowSection, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewPageHeader } from "./ui/panel/ViewFrame";

/**
 * **Needs you** (#3689; spec #3679, the amendment's S9, `prototype/studio-secondary-pages` A):
 * what in a studio's skills needs a decision, grouped by kind, each with why and one fix, the
 * item chosen beside the list. The core works the items out (`skillstudio::needs`), so the
 * navigation's count, the page and the summary lines count the same ones. Nothing here is
 * broken, so an item is drawn in the attention tone, never as trouble.
 */

/** **What each kind is called**, as the page groups them, in the core's order. */
export const NEED_KIND_NAMES: Record<NeedKind, string> = {
  review: "Changes to review",
  clash: "Clashes",
  problem: "Problems in a skill",
  pins: "Pins no chat reads",
  "tidy-up": "Tidy up",
};

/** What the page says it holds, under its name. */
export const NEEDS_YOU_LEAD =
  "What in these skills needs a decision, each with its fix. When nothing does, the page leaves the navigation.";

/** Its first sentence: what a row says under its name. */
function firstSentence(text: string): string {
  const end = text.search(/[.!?](\s|$)/);
  return end === -1 ? text : text.slice(0, end + 1);
}

/** The items by kind, in the order the core gave them, which is the kinds' order. */
function byKind(needs: readonly NeedsYou[]): [NeedKind, NeedsYou[]][] {
  const groups = new Map<NeedKind, NeedsYou[]>();
  for (const one of needs) {
    const group = groups.get(one.kind);
    if (group === undefined) groups.set(one.kind, [one]);
    else group.push(one);
  }
  return [...groups];
}

/** **The item the page shows beside its list**: the one chosen, or else the first. */
export function needShown(needs: readonly NeedsYou[], chosen: string | undefined) {
  return needs.find((one) => one.id === chosen) ?? needs[0];
}

/** **The list of Needs you**: a group per kind, with its count, and a row per item. */
export function NeedsYouList({
  needs,
  chosen,
  onChoose,
}: {
  needs: readonly NeedsYou[];
  /** The item the person chose, by its id: none until they do, so that pressing the first
   *  opens it on a narrow tab too (beside the list, the first is shown until then). */
  chosen: string | undefined;
  onChoose: (id: string) => void;
}) {
  if (needs.length === 0)
    return (
      <EmptyState
        headline="Nothing needs you"
        body="Changes to review, clashes, problems in a skill, pins no chat reads and tidy-ups show up here."
        testid="skill-needs-none"
      />
    );
  return (
    <List
      aria-label="What needs you"
      className="skill-needs"
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={chosen === undefined ? new Set() : new Set([chosen])}
      onSelectionChange={(keys: Selection) => {
        const [first] = keys === "all" ? [] : [...keys];
        if (first !== undefined) onChoose(String(first));
      }}
    >
      {byKind(needs).map(([kind, items]) => (
        <ListRowSection
          key={kind}
          id={kind}
          title={NEED_KIND_NAMES[kind]}
          count={items.length}
          aria-label={NEED_KIND_NAMES[kind]}
        >
          {items.map((one) => (
            <ListRow key={one.id} id={one.id} textValue={one.title} data-testid={`need-${one.id}`}>
              <RowLabel>{one.title}</RowLabel>
              {one.tag !== null && (
                <RowTrail>
                  <Badge>{one.tag}</Badge>
                </RowTrail>
              )}
              <RowBelow clamp={1}>{firstSentence(one.reason)}</RowBelow>
            </ListRow>
          ))}
        </ListRowSection>
      ))}
    </List>
  );
}

/**
 * **One item of Needs you, beside the list**: its name and where it is, then why it needs the
 * person in the attention tone with its one fix, and what the fix does. The old plugin keys may
 * also be left as they are, which says so once, as their Ask in the Inbox did.
 */
export function NeedsYouItem({
  need,
  busy,
  onFix,
  onLeave,
}: {
  need: NeedsYou;
  busy: boolean;
  onFix: (need: NeedsYou) => void;
  /** Leaves the old plugin keys as they are: their Ask's Dismiss, beside Remove. */
  onLeave: () => void;
}) {
  const fixes: [NoticeAction, ...NoticeAction[]] = [
    { label: need.fix_label, busy, primary: true, onPress: () => onFix(need) },
  ];
  if (need.fix.kind === "remove-old-keys")
    fixes.push({ label: "Leave them", busy, onPress: onLeave });
  return (
    <div className="skill-studio-item" data-testid="skill-need" data-need={need.id}>
      <ViewPageHeader
        name={need.title}
        tag={need.tag === null ? undefined : <Badge>{need.tag}</Badge>}
        description={NEED_KIND_NAMES[need.kind]}
      />
      <Notice cause={`skill-need:${need.id}`} tone="attention" label={need.title} fixes={fixes}>
        {need.reason}
      </Notice>
      <p className="honest">{need.fix_does}</p>
    </div>
  );
}
