// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A hibernated chat's pane** (SC-4b, #7631; ADR 0121 as amended 2026-11-25): what the chat
 * showed as it was hibernated, kept by the core masked and within 1 MiB
 * (`purlis_core::hibernatedscreens`), drawn read-only by the terminal's read-only mode in place
 * of its live terminal, under the band that offers Resume (`HibernatedNotice`). Where none was
 * kept (it could not be taken or masked, or the file is missing, corrupt or too large) it says
 * so, and Resume still works.
 *
 * **Typing on the kept screen wakes the chat** (SC-4j, #7639): each key goes to the core as a
 * live pane's does, which holds it, wakes the chat, and types it into the woken run once that
 * run is ready.
 */
import type { ReactNode } from "react";
import { commands, type PlaneId } from "./bindings";
import { useHibernated } from "./HibernatedNotice";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";
import { useRead } from "./SkillPanels";
import { textSizes } from "./textSize";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** The size a kept screen is drawn at, as *Read it*'s is. */
const SCREEN_COLUMNS = 120;
const SCREEN_ROWS = 40;

/** What the screen is named. */
export const HIBERNATED_SCREEN_LABEL = "What this chat showed as it was hibernated";

/** What the pane says of the kept screen beside it. */
export const HIBERNATED_SCREEN_NOTE =
  "A copy of what this chat showed as it was hibernated. Typing here wakes it, and it reads what you type once it is ready.";

/** What the pane says where no screen was kept. */
export const NONE_KEPT = "No copy of this chat's screen was kept";
export const NONE_KEPT_WHY =
  "Its conversation is kept all the same. Resume starts it again on its conversation.";

/**
 * **`live`, unless chat `session` is hibernated**, in which case the screen kept for it. Read
 * when the pane mounts and each time a chat's hibernation moves, as the band is.
 */
export function HibernatedOr({
  plane,
  session,
  children,
}: {
  plane: PlaneId;
  session: number;
  children: ReactNode;
}) {
  const hibernated = useHibernated(plane, session);
  if (hibernated === null) return <>{children}</>;
  return <HibernatedScreen plane={plane} session={session} />;
}

/** The screen kept for hibernated chat `session`, read from the core. */
export function HibernatedScreen({ plane, session }: { plane: PlaneId; session: number }) {
  const screen = useRead(() => commands.hibernatedChatScreen(plane, session), [plane, session]);
  // Not read yet, or not read at all, reads as nothing kept: Resume is the band's either way.
  if (screen.data === undefined && screen.refused === undefined) return null;
  return (
    <HibernatedScreenShown
      screen={screen.data ?? null}
      onTyped={(text) => {
        // Refused or not, the core says it on the pane: a send it cannot take is told there.
        void commands.sendInput(plane, session, text).catch(() => undefined);
      }}
    />
  );
}

/** The kept screen, `null` where none was kept: what the story draws. `onTyped` is where a key
 *  typed on it goes. */
export function HibernatedScreenShown({
  screen,
  onTyped,
}: {
  screen: string | null;
  onTyped?: (text: string) => void;
}) {
  return (
    <section className="view-pane view-alone" data-testid="hibernated-screen">
      {screen === null ? (
        <EmptyState
          variant="panel"
          headline={NONE_KEPT}
          body={NONE_KEPT_WHY}
          testid="hibernated-screen-none"
        />
      ) : (
        <ReadOnlyTerminal
          screen={screen}
          label={HIBERNATED_SCREEN_LABEL}
          note={HIBERNATED_SCREEN_NOTE}
          cols={SCREEN_COLUMNS}
          rows={SCREEN_ROWS}
          fontSize={textSizes().terminal}
          onTyped={onTyped}
        />
      )}
    </section>
  );
}
