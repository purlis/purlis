// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * ***Read it*'s tab** (#6214): what a dispatched task's chat showed as it closed, as the app kept
 * it (`purlis_core::chatscreens`), drawn read-only by the terminal's read-only mode
 * (`ReadOnlyTerminal`, #3676). A view tab keyed by the chat's ULID, so it outlives the card that
 * opened it and is put back by a launch. Where the screen is no longer kept (the setting was
 * turned off, or its record was collected), it says so.
 */
import { commands, type PlaneId } from "./bindings";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { NotRead, ReadingRows, useRead } from "./SkillPanels";
import { textSizes } from "./textSize";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SCREEN_NOT_KEPT } from "./closedChat";

/** The size a kept screen is drawn at: a chat's own pane's, wide. */
const SCREEN_COLUMNS = 120;
const SCREEN_ROWS = 40;

/** What the tab says of itself beside the screen. */
export const CHAT_SCREEN_NOTE = "What this task's chat showed as it closed. It takes no input.";

export function ChatScreenTab({
  plane,
  chat,
  title,
}: {
  plane: PlaneId;
  /** The chat's ULID. */
  chat: string;
  /** The tab's title: "#641 · Signed commits #5607 (read-only)". */
  title: string;
}) {
  const screen = useRead(() => commands.chatScreen(plane, chat), [plane, chat]);
  return (
    <section className="view-pane view-alone" data-testid="view-pane-chat-screen" aria-label={title}>
      {screen.refused !== undefined ? (
        <NotRead what="the chat's screen" why={screen.refused} />
      ) : screen.data === undefined ? (
        <ReadingRows what="the chat's screen" testid="chat-screen-reading" />
      ) : screen.data === null ? (
        <EmptyState
          variant="panel"
          headline="This chat's screen is not kept"
          body={SCREEN_NOT_KEPT}
          testid="chat-screen-not-kept"
        />
      ) : (
        <ReadOnlyTerminal
          screen={screen.data}
          label={title}
          note={CHAT_SCREEN_NOTE}
          cols={SCREEN_COLUMNS}
          rows={SCREEN_ROWS}
          fontSize={textSizes().terminal}
        />
      )}
    </section>
  );
}
