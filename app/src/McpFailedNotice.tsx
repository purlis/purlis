// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useContext, useEffect, useState } from "react";
import { commands, type McpFailed, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice, NoticeOf } from "./Notice";

/**
 * **A persona's MCP server that could not start, on its chat's tab** (qw381, ruling T-3).
 *
 * Claude Code starts the server as the chat starts, and purlis tries a failed start once more
 * by itself. When that fails too, the core keeps the failure and says so
 * (`chat-mcp-failed`), and this says it on the chat's pane: which server, the persona, and the
 * server's own error, quoted as data (bounded and masked by the core). It offers
 *
 * - **Retry**: the chat's restart on its conversation, once its turn has ended, as Restart chat
 *   does (`onRestart`). Claude Code takes no outside word to reconnect one server, so
 *   every server starts again with the chat;
 * - **Dismiss**: puts it away.
 *
 * The chat itself is told at its next turn, in purlis's words, so it stops guessing.
 */
export function McpFailedNotice({
  plane,
  session,
  onRestart,
}: {
  plane: PlaneId;
  session: number;
  /** Asks for this chat's restart on its conversation: the pane's Restart chat. */
  onRestart: () => void;
}) {
  const failed = useMcpFailures(plane, session);
  const offScreen = useContext(NoticeOf) !== null;
  const newest = failed.list[failed.list.length - 1];
  if (newest === undefined) return null;
  const { server, persona, error } = newest;
  const chat = offScreen ? "that chat" : "this chat";
  const more = failed.list.length - 1;

  const putAway = () => {
    void commands
      .putAwayMcpFailure(plane, session, server)
      .catch(() => {})
      .finally(failed.read);
  };

  return (
    <Notice
      cause={`mcp-failed:${session}:${server}`}
      at="pane"
      tone="trouble"
      label="MCP server"
      persona={persona}
      fixes={[
        {
          label: "Retry",
          onPress: () => {
            onRestart();
            putAway();
          },
        },
      ]}
      onDismiss={putAway}
      under={
        <p className="ui-text-secondary">
          Its error: <span className="ui-text-code">{error}</span>
        </p>
      }
    >
      MCP server <code className="ui-text-code">{server}</code> could not start, twice, so its tools
      are not in {chat}. Retry restarts {chat} on its conversation once its turn has ended.
      {more > 0 && ` ${more} more ${more === 1 ? "server" : "servers"} behind this one.`}
    </Notice>
  );
}

/**
 * The servers chat `session` could not start, read when the pane mounts and each time the core
 * says one failed, and `read` to read them again after a press. A list that cannot be read shows
 * nothing: the chat is still told.
 */
export function useMcpFailures(
  plane: PlaneId,
  session: number,
): { list: readonly McpFailed[]; read: () => void } {
  const [list, setList] = useState<readonly McpFailed[]>([]);
  const read = useCallback(() => {
    void commands
      .mcpFailures(plane, session)
      .then((answer) => {
        if (answer.status === "ok" && Array.isArray(answer.data)) setList(answer.data);
      })
      .catch(() => {});
  }, [plane, session]);
  useEffect(() => {
    read();
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<McpFailed>("chat-mcp-failed", (event) => {
          if (event.payload.plane !== plane || event.payload.session !== session) return;
          read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, session, read]);
  return { list, read };
}
