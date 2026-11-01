// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { RotateCw } from "lucide-react";
import { commands, type DeletedSkill } from "./bindings";
import { counted } from "./dispatches";
import { Field, SettingRow } from "./settings/components";
import { NAME_RESERVED, copyNameOf, isReservedName } from "./SkillCopy";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { List, ListRow, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewPageHeader } from "./ui/panel/ViewFrame";

/** How many days are left to bring it back, in words. */
export function daysLeft(one: DeletedSkill): string {
  return `${counted(one.days_left, "day", "days")} left to bring it back`;
}

/**
 * **The Deleted page's list** (S9, #3691; `prototype/studio-secondary-pages`, variant A): a row
 * of each skill deleted in the last 30 days, its name and skill level, with the days left to
 * bring it back under them. The one chosen opens beside the list ({@link DeletedPage}).
 */
export function DeletedList({
  deleted,
  chosen,
  onChoose,
}: {
  deleted: readonly DeletedSkill[];
  /** The deleted skill chosen, by its id. */
  chosen: string | undefined;
  onChoose: (id: string) => void;
}) {
  return (
    <List
      aria-label="Deleted skills"
      className="skill-library"
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={chosen === undefined ? new Set() : new Set([chosen])}
      onSelectionChange={(keys: Selection) => {
        const [first] = keys === "all" ? [] : [...keys];
        if (first !== undefined) onChoose(String(first));
      }}
    >
      {deleted.map((one) => (
        <ListRow
          key={one.id}
          id={one.id}
          textValue={one.name}
          data-testid={`deleted-${one.name}`}
          data-row={one.id}
        >
          <RowLabel>{one.name}</RowLabel>
          <RowTrail>
            <Badge>{one.label}</Badge>
          </RowTrail>
          <RowBelow clamp={1}>{daysLeft(one)}</RowBelow>
        </ListRow>
      ))}
    </List>
  );
}

/** Its last text, as it was read: not yet, read, or why it could not be. */
type Read = { text: string } | { refused: string } | undefined;

/** What a deleted skill whose name a built-in skill took is told (#3729). */
export const NAME_TAKEN = "Its name belongs to purlis now, so it comes back under a new name.";

/**
 * **A deleted skill's page** (S9, #3691), beside the Deleted list: what bringing it back does
 * (it comes back with its history and grades, and chats started after that get it again), the
 * days left, **Bring back**, and **its last text**, read only when the person opens it.
 *
 * **One whose name a built-in skill took** (#3729: a newer purlis ships one of that name) comes
 * back only under a new name: the page asks for it, starting as the built-in copy's would, and
 * refuses a name of purlis's own as it is typed, in the core's words; the core checks it again.
 */
export function DeletedPage({
  plane,
  deleted,
  busy,
  onBringBack,
}: {
  plane: string;
  deleted: DeletedSkill;
  busy: boolean;
  /** Bring it back: `under` the new name, where its own is a built-in skill's. */
  onBringBack: (one: DeletedSkill, under?: string) => void;
}) {
  const [read, setRead] = useState<Read>();
  const [asked, setAsked] = useState(false);
  const [under, setUnder] = useState(() => copyNameOf(deleted.name));
  const wanted = under.trim();
  const reserved = deleted.reserved && isReservedName(wanted);
  const ready =
    !busy && (!deleted.reserved || (wanted !== "" && wanted !== deleted.name && !reserved));
  const readIt = () => {
    setAsked(true);
    setRead(undefined);
    void commands
      .skillDeletedText(plane, deleted.id)
      .then((answer) =>
        setRead(answer.status === "ok" ? { text: answer.data } : { refused: answer.error }),
      )
      .catch((err: unknown) => setRead({ refused: String(err) }));
  };
  const days = counted(deleted.days_left, "more day", "more days");
  const after = deleted.in_history
    ? "After that it leaves this list; the project's history still holds it."
    : "After that it is removed from this machine for good.";
  return (
    <div className="skill-studio-item" data-testid="skill-studio-deleted">
      <ViewPageHeader
        name={deleted.name}
        tag={<Badge>{deleted.label}</Badge>}
        description={`Deleted from the ${deleted.label} skills. It can be brought back for ${days}, with its history and grades. ${after}`}
        actions={
          <Button
            variant="primary"
            aria-label={
              deleted.reserved
                ? `Bring back ${deleted.name} as ${wanted}`
                : `Bring back ${deleted.name}`
            }
            isDisabled={!ready}
            onPress={() => {
              if (!ready) return;
              if (deleted.reserved) onBringBack(deleted, wanted);
              else onBringBack(deleted);
            }}
          >
            Bring back
          </Button>
        }
      />
      <div>
        {deleted.reserved && (
          <>
            <p className="honest" data-testid="deleted-name-taken">
              {NAME_TAKEN}
            </p>
            <SettingRow
              label="New name"
              help="Lowercase letters, digits and hyphens, not starting with purlis-."
              control={(ids) => <Field kind="text" ids={ids} value={under} onChange={setUnder} />}
            />
            {reserved && (
              <p className="trouble" role="alert" data-testid="deleted-name-reserved">
                {NAME_RESERVED}
              </p>
            )}
          </>
        )}
        <p>Chats started after it is brought back get it again.</p>
        <Disclosure
          label="Show its last text"
          onExpandedChange={(open) => {
            if (open && !asked) readIt();
          }}
          data-testid="deleted-last-text"
        >
          {read === undefined ? (
            <p className="pending">Reading its last text…</p>
          ) : "text" in read ? (
            <pre className="skill-deleted-text">{read.text}</pre>
          ) : (
            <div>
              <p>{`purlis could not read its last text: ${read.refused}`}</p>
              <ButtonRow size="sm">
                <Button icon={<RotateCw aria-hidden="true" />} onPress={readIt}>
                  Try again
                </Button>
              </ButtonRow>
            </div>
          )}
        </Disclosure>
      </div>
    </div>
  );
}
