// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useMemo, useState, type ReactNode } from "react";
import { fleetKey, inFleetOrder, type FleetChat } from "./everyChat";
import { ChatActions } from "./ChatActions";
import { rowTimeSaid } from "./chatsList";
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
 *   in the view, as Snooze all does on Asks; Snooze answers nothing, so it does not.
 * - **Filtered to one state** where a link asked for it (#6020's "7 chats working"), with Show
 *   all to take the filter off.
 * - **Drawn in part** (`drawnInPart`): only the rows in sight, however many projects are open.
 */
export type FleetDoes = {
  /** Go to chat: its project brought forward where it is not, in whichever window holds it. */
  go: (chat: FleetChat) => void;
  /** Answer: Asks opened at the chat's asks. */
  answer: (chat: FleetChat) => void;
  /** Snoozes each chat's asks; answers what went wrong, or nothing. */
  snooze: (chats: readonly FleetChat[]) => Promise<string | undefined>;
  /** Stops each chat (a task by its own Stop, which lets it report); answers what went wrong. */
  stop: (chats: readonly FleetChat[]) => Promise<string | undefined>;
};

/** What the view's title says, and its switch. */
export const ALL_CHATS = "All chats";

/** `3 chats`, `1 chat`. */
const chatsSaid = (count: number) => (count === 1 ? "1 chat" : `${count} chats`);

export function AllChats({
  chats,
  filter,
  onFilter,
  onLeave,
  does,
  lineOf,
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
  /** What a chat is doing now, read live, where the chat is in this window (`ChatDoingLine`). */
  lineOf?: (chat: FleetChat) => ReactNode;
}) {
  const listed = useMemo(
    () => inFleetOrder(filter === undefined ? chats : chats.filter((one) => one.state === filter)),
    [chats, filter],
  );
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set());
  /** What is chosen and still listed: a chat that closed or left the filter is not. */
  const picked = useMemo(() => listed.filter((one) => chosen.has(fleetKey(one))), [chosen, listed]);
  const [asking, setAsking] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  const stoppable = picked.filter((one) => one.stoppable);
  const snoozable = picked.filter((one) => one.asks.length > 0);

  const done = (said: string | undefined) => {
    setTrouble(said);
    if (said === undefined) setChosen(new Set());
  };
  const stop = async () => {
    setAsking(false);
    done(await does.stop(stoppable));
  };
  const snooze = async () => done(await does.snooze(snoozable));
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
          {picked.length > 0 && (
            <div className="all-chats-bulk" role="group" aria-label="Chosen chats">
              {asking ? (
                <>
                  <p className="ui-text-secondary">
                    {`Stop ${chatsSaid(stoppable.length)}? A task gets one short turn to write what it did.`}
                  </p>
                  <ButtonRow size="sm">
                    <Button variant="danger" onPress={() => void stop()}>
                      Stop
                    </Button>
                    <Button onPress={() => setAsking(false)}>Cancel</Button>
                  </ButtonRow>
                </>
              ) : (
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
                          : `Until each asks something new: ${chatsSaid(snoozable.length)}`
                      }
                      onPress={() => void snooze()}
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
                      onPress={() => setAsking(true)}
                    >
                      Stop
                    </Button>
                    <Button variant="quiet" onPress={() => setChosen(new Set())}>
                      Clear
                    </Button>
                  </ButtonRow>
                </>
              )}
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
          <FleetRow key={fleetKey(chat)} chat={chat} does={does} line={lineOf?.(chat)} />
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
  line,
}: {
  chat: FleetChat;
  does: FleetDoes;
  line: ReactNode;
}) {
  const where = `${CHAT_STATE_NAMES[chat.state]} · ${chat.project} · ${chat.workspace}`;
  // What it is doing now, read live, else its last line as its project told it.
  const below = line ?? chat.line;
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
      {below != null && below !== "" && (
        <RowBelow clamp={1} tipWhenCut className="all-chats-line">
          {below}
        </RowBelow>
      )}
      <RowActions>
        <ChatActions
          who={chat.name}
          answer={chat.state === "needs" ? { onPress: () => does.answer(chat) } : undefined}
          onGo={() => does.go(chat)}
          onSnooze={chat.asks.length > 0 ? () => void does.snooze([chat]) : undefined}
          onStop={chat.stoppable ? () => void does.stop([chat]) : undefined}
        />
      </RowActions>
    </ListRow>
  );
});

/** How long it has been in its state, on the row's own clock: none where nobody saw it come. */
function InState({ since }: { since: number | null }) {
  const now = useNow();
  if (since === null) return null;
  const seconds = Math.max(0, Math.floor((now - since) / 1000));
  return (
    <span className="chat-time ui-text-meta">
      {rowTimeSaid(seconds, new Date(now))}
    </span>
  );
}
