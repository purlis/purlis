// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useMemo, useState, useSyncExternalStore, type ComponentProps } from "react";
import { DoingsHere, saidWhole, useDoingSaid, type DoingsOf } from "./chatDoing";
import { ChatsHere, type Chats } from "./chatState";
import { fleetKey, inFleetOrder, type FleetChat } from "./everyChat";
import { ChatActions } from "./ChatActions";
import { rowTimeSaid } from "./chatsList";
import { snoozeEnds, type SnoozeFor } from "./AsksSnooze";
import { AskWeek, useAskWeek } from "./AskWeek";
import type { AskMeasure } from "./bindings";
import { Notice } from "./Notice";
import { useNow } from "./rowClock";
import { CHAT_STATE_NAMES, type ChatState } from "./shownState";
import { Button, ButtonRow, ToggleButton } from "./ui/button/Button";
import { ChatMark } from "./ui/chat-mark/ChatMark";
import { Checkbox } from "./ui/choice/Checkbox";
import { List, ListRow, type Selection } from "./ui/list/List";
import { RowActions, RowBelow, RowLabel, RowLead, RowSummary, RowTrail } from "./ui/list/Row";
import { PanelSection as Section, PanelTitle } from "./ui/panel/Panel";

/**
 * **All chats** (ADR 0121 decisions 18 and 21, #6013; CONTEXT.md, **All chats**): the Chats
 * sidebar's mode that lists every chat in every open project, switched on and off at its head.
 *
 * - **A row per chat**: its mark (`ChatMark`) and name, how long it has been in its state, its
 *   state by name with its project and workspace, its last line, and the chat's actions in the
 *   one order (`ChatActions`). A press of the row is Go to chat.
 * - **Waiting on you first, then Stuck?, then the rest** (`everyChat.inFleetOrder`).
 * - **Several at once**: chosen by their boxes (or Space, Shift and the arrows, the list's own
 *   keys), they can be stopped or snoozed together, and nothing else. Stop asks once, in a line
 *   in the view, as Snooze all does on Asks. Snooze offers Snooze all's choices (#6018): until
 *   each asks again, for 1 hour, or until tomorrow, whichever comes first.
 * - **Filtered to one state** where a link asked for it (#6020's "7 chats working"), with Show
 *   all to take the filter off.
 * - **Drawn in part** (`drawnInPart`): only the rows in sight, however many projects are open.
 * - **Each project's week of asking** at its head (`AskWeek`, #6021): a quiet line per project.
 */
export type FleetDoes = {
  /** Go to chat: its project brought forward where it is not, in whichever window holds it. */
  go: (chat: FleetChat) => void;
  /** Answer: Asks opened at the chat's asks. */
  answer: (chat: FleetChat) => void;
  /** Snoozes each chat's asks, until `until` (seconds since 1970) at the latest where given,
   *  as Snooze all on Asks does (#6018); answers what went wrong, or nothing. */
  snooze: (chats: readonly FleetChat[], until: number | null) => Promise<string | undefined>;
  /** Stops each chat, already asked about in the view (a task by its own Stop, which lets it
   *  report first); answers what went wrong. */
  stop: (chats: readonly FleetChat[]) => Promise<string | undefined>;
};

/** **A project's own stores, lent** (#6013): what a row of a chat in this window reads live from,
 *  as that project's own rows do. A chat in another window has none here. */
export type Lent = { chats: Chats; doings: DoingsOf };

/**
 * **Every chat, as the window hands it to each project's view** (#6013): the entries of every
 * open project, in this window and the others; what the rows do; and each project's stores where
 * this window holds it.
 */
export type EveryChat = {
  /** The entries, held apart from what draws a project's view: a chat that moves draws All
   *  chats again, and no project's view (`MovedRenders.test.tsx`). */
  store: FleetStore;
  does: FleetDoes;
  lentOf: (plane: string) => Lent | undefined;
};

/** **Every chat's entry, held** (#6013): set by the window, read by All chats alone. */
export type FleetStore = {
  get: () => readonly FleetChat[];
  set: (chats: readonly FleetChat[]) => void;
  subscribe: (listener: () => void) => () => void;
};

const NO_CHATS: readonly FleetChat[] = [];

/** A store holding no chat yet. */
export function fleetStore(): FleetStore {
  let held = NO_CHATS;
  const listeners = new Set<() => void>();
  return {
    get: () => held,
    set: (chats) => {
      if (chats === held) return;
      held = chats;
      for (const listener of [...listeners]) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
}

/** **All chats, reading every chat off the window's store**: what a project's view draws. */
export function AllChatsOf({
  every,
  ...rest
}: { every: EveryChat } & Omit<ComponentProps<typeof AllChats>, "chats" | "does" | "lentOf">) {
  const chats = useSyncExternalStore(every.store.subscribe, every.store.get);
  const measures = useAskWeek();
  return (
    <AllChats {...rest} chats={chats} does={every.does} lentOf={every.lentOf} measures={measures} />
  );
}

/** A project's name by its plane: as its chats name it, else its folder's. */
function projectOf(chats: readonly FleetChat[], plane: string): string {
  return (
    chats.find((one) => one.plane === plane)?.project ??
    plane.split(/[\\/]/).filter(Boolean).pop() ??
    plane
  );
}

/** What the view's title says, and its switch. */
export const ALL_CHATS = "All chats";

/** `3 chats`, `1 chat`. */
const chatsSaid = (count: number) => (count === 1 ? "1 chat" : `${count} chats`);

/** What Stop asks: one chat by its name, several by how many, and what a task gets first. */
function stopSaid(chats: readonly FleetChat[]): string {
  const which = chats.length === 1 ? chats[0].name : chatsSaid(chats.length);
  const tasks = chats.some((one) => one.task)
    ? " A task gets one short turn to write what it did."
    : "";
  return `Stop ${which}? It ends the program it runs.${tasks}`;
}

export function AllChats({
  chats,
  filter,
  onFilter,
  onLeave,
  does,
  lentOf,
  measures = [],
}: {
  /** Every chat of every open project, in any order. */
  chats: readonly FleetChat[];
  /** The one state listed, where a link asked for it. */
  filter?: ChatState;
  /** Takes the filter off, or puts one on. */
  onFilter: (state: ChatState | undefined) => void;
  /** The switch turned off: back to this project's chats. */
  onLeave: () => void;
  does: FleetDoes;
  /** Each project's stores, where this window holds it: what a working chat's line is read
   *  live from. */
  lentOf?: (plane: string) => Lent | undefined;
  /** Each open project's week of asking (#6021). */
  measures?: readonly AskMeasure[];
}) {
  const listed = useMemo(
    () => inFleetOrder(filter === undefined ? chats : chats.filter((one) => one.state === filter)),
    [chats, filter],
  );
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  /** What is chosen and still listed: a chat that closed or left the filter is not. */
  const picked = useMemo(() => listed.filter((one) => chosen.has(fleetKey(one))), [chosen, listed]);
  /** The chats a Stop asks about, a row's one or the chosen ones, while it asks. */
  const [toStop, setToStop] = useState<readonly FleetChat[]>();
  /** The chosen chats a Snooze asks how long about, while it asks. */
  const [toSnooze, setToSnooze] = useState<readonly FleetChat[]>();
  const [trouble, setTrouble] = useState<string>();
  const stoppable = picked.filter((one) => one.stoppable);
  const snoozable = picked.filter((one) => one.asks.length > 0);

  const done = (said: string | undefined) => {
    setTrouble(said);
    if (said === undefined) setChosen(new Set());
  };
  const stop = async () => {
    const chats = toStop ?? [];
    setToStop(undefined);
    done(await does.stop(chats));
  };
  const snooze = async (choice: SnoozeFor) => {
    const chats = toSnooze ?? [];
    setToSnooze(undefined);
    done(await does.snooze(chats, snoozeEnds(choice, new Date())));
  };
  const choose = (keys: Selection) =>
    setChosen(new Set(keys === "all" ? listed.map(fleetKey) : [...keys].map(String)));

  return (
    <Section
      view
      className="chats-section all-chats ui-text-body"
      aria-label={ALL_CHATS}
      top={
        <>
          <div className="chats-top">
            <PanelTitle id="all-chats-title" className="side-title">
              {ALL_CHATS}
            </PanelTitle>
            <AllChatsSwitch on onChange={(on) => !on && onLeave()} />
          </div>
          <AskWeek measures={measures} projectOf={(plane) => projectOf(chats, plane)} />
          {filter !== undefined && (
            <div className="all-chats-filter">
              <p className="ui-text-secondary" role="status">
                {`Only chats that are ${CHAT_STATE_NAMES[filter]}.`}
              </p>
              <Button size="sm" variant="quiet-link" onPress={() => onFilter(undefined)}>
                Show all
              </Button>
            </div>
          )}
          {trouble !== undefined && (
            <Notice
              cause="all-chats-trouble"
              tone="trouble"
              onDismiss={() => setTrouble(undefined)}
            >
              {trouble}
            </Notice>
          )}
          {toStop !== undefined && (
            // **Stop asks once, in the view** (#6013), for a row's chat or the chosen ones alike:
            // never a dialog per chat, which only the project in front could draw.
            <div className="all-chats-bulk" role="group" aria-label="Stop chats">
              <p className="ui-text-secondary">{stopSaid(toStop)}</p>
              <ButtonRow size="sm">
                <Button variant="danger" onPress={() => void stop()}>
                  Stop
                </Button>
                <Button onPress={() => setToStop(undefined)}>Cancel</Button>
              </ButtonRow>
            </div>
          )}
          {toSnooze !== undefined && (
            // **Snooze asks how long, in the view** (#6018): the choices Snooze all offers on Asks.
            <div className="all-chats-bulk" role="group" aria-label="Snooze chats">
              <p className="ui-text-secondary">
                {`Snooze ${chatsSaid(toSnooze.length)}? Each comes back when it asks something new, or at the time you choose if that comes first.`}
              </p>
              <ButtonRow size="sm">
                <Button variant="primary" onPress={() => void snooze("new-ask")}>
                  Until it asks again
                </Button>
                <Button onPress={() => void snooze("hour")}>For 1 hour</Button>
                <Button onPress={() => void snooze("tomorrow")}>Until tomorrow</Button>
                <Button onPress={() => setToSnooze(undefined)}>Cancel</Button>
              </ButtonRow>
            </div>
          )}
          {picked.length > 0 && toStop === undefined && toSnooze === undefined && (
            <div className="all-chats-bulk" role="group" aria-label="Chosen chats">
              <>
                <p className="ui-text-secondary" role="status">
                  {`${chatsSaid(picked.length)} chosen`}
                </p>
                <ButtonRow size="sm">
                  <Button
                    isDisabled={snoozable.length === 0}
                    tooltip={
                      snoozable.length === 0
                        ? "None of them asks anything, so there is nothing to snooze."
                        : `Snooze ${chatsSaid(snoozable.length)}`
                    }
                    onPress={() => setToSnooze(snoozable)}
                  >
                    Snooze
                  </Button>
                  <Button
                    isDisabled={stoppable.length === 0}
                    tooltip={
                      stoppable.length === 0
                        ? "They have all ended, so there is nothing to stop."
                        : undefined
                    }
                    onPress={() => setToStop(stoppable)}
                  >
                    Stop
                  </Button>
                  <Button variant="quiet" onPress={() => setChosen(new Set())}>
                    Clear
                  </Button>
                </ButtonRow>
              </>
            </div>
          )}
        </>
      }
    >
      <List
        aria-label={ALL_CHATS}
        className="all-chats-list"
        drawnInPart
        selectionMode="multiple"
        selectionBehavior="toggle"
        selectedKeys={chosen as Set<string>}
        onSelectionChange={choose}
        onAction={(key) => {
          const chat = listed.find((one) => fleetKey(one) === key);
          if (chat !== undefined) does.go(chat);
        }}
        items={listed.map((chat) => ({ id: fleetKey(chat), chat }))}
        empty={
          <p className="ui-text-secondary all-chats-none">
            {filter === undefined
              ? "No chat is open in any project."
              : `No chat is ${CHAT_STATE_NAMES[filter]}.`}
          </p>
        }
      >
        {({ chat }) => (
          <FleetRow
            key={fleetKey(chat)}
            chat={chat}
            does={does}
            onStop={setToStop}
            lent={lentOf?.(chat.plane)}
          />
        )}
      </List>
    </Section>
  );
}

/** **The switch at the Chats sidebar's head** (#6013): this project's chats, or All chats. */
export function AllChatsSwitch({ on, onChange }: { on: boolean; onChange: (on: boolean) => void }) {
  return (
    <ToggleButton
      size="sm"
      isSelected={on}
      onPress={() => onChange(!on)}
      tooltip="Every chat in every open project"
    >
      {ALL_CHATS}
    </ToggleButton>
  );
}

/** One chat's row. */
const FleetRow = memo(function FleetRow({
  chat,
  does,
  onStop,
  lent,
}: {
  chat: FleetChat;
  does: FleetDoes;
  /** Its Stop: asked first, in the view. */
  onStop: (chats: readonly FleetChat[]) => void;
  lent: Lent | undefined;
}) {
  const where = `${CHAT_STATE_NAMES[chat.state]} · ${chat.project} · ${chat.workspace}`;
  return (
    <ListRow
      id={fleetKey(chat)}
      variant="card"
      textValue={`${chat.name}, ${where}`}
      data-state={chat.state}
    >
      <RowLead>
        <Checkbox slot="selection" aria-label={`Choose ${chat.name}, ${chat.project}`} />
        <ChatMark state={chat.state} word={chat.word} />
      </RowLead>
      {/* A chat's name is data: drawn as text. */}
      <RowLabel>{chat.name}</RowLabel>
      <RowTrail>
        <InState since={chat.since} />
      </RowTrail>
      <RowSummary>{where}</RowSummary>
      {lent === undefined ? (
        <LastLine said={chat.line} />
      ) : (
        <ChatsHere.Provider value={lent.chats}>
          <DoingsHere.Provider value={lent.doings}>
            <DoingOrLast session={chat.session} said={chat.line} />
          </DoingsHere.Provider>
        </ChatsHere.Provider>
      )}
      <RowActions>
        <ChatActions
          who={chat.name}
          answer={chat.state === "needs" ? { onPress: () => does.answer(chat) } : undefined}
          onGo={() => does.go(chat)}
          onSnooze={chat.asks.length > 0 ? () => void does.snooze([chat], null) : undefined}
          onStop={chat.stoppable ? () => onStop([chat]) : undefined}
        />
      </RowActions>
    </ListRow>
  );
});

/** **Its last line**, cut at one line, whole on hover: data, drawn as text. */
function LastLine({ said }: { said: string | null }) {
  if (said === null || said === "") return null;
  return (
    <RowBelow clamp={1} tipWhenCut className="all-chats-line">
      {said}
    </RowBelow>
  );
}

/** What a working chat of this window is doing now ("running cargo"), read live off its project's
 *  stores as its own row reads it (`useDoingSaid`), else its last line. */
function DoingOrLast({ session, said }: { session: number; said: string | null }) {
  const doing = useDoingSaid(session);
  return <LastLine said={doing === undefined ? said : saidWhole(doing)} />;
}

/** How long it has been in its state, on the row's own clock: none where nobody saw it come. */
function InState({ since }: { since: number | null }) {
  const now = useNow();
  if (since === null) return null;
  const seconds = Math.max(0, Math.floor((now - since) / 1000));
  return <span className="chat-time ui-text-meta">{rowTimeSaid(seconds, new Date(now))}</span>;
}
