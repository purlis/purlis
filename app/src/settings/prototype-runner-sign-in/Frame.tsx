// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * The host page each variant is drawn in: Settings › You › This machine › Pools in the real
 * settings layout, or, for the Move to runner moment, a stand-in of a chat's pane. And a scrim
 * to draw a dialog over either, in place, so the prototype's bar stays pressable.
 */
import { useState, type ReactNode } from "react";
import { SettingGroup, SettingRow, SettingsLayout } from "../components";
import { HarnessIcon } from "../../ui/harness-icon/HarnessIcon";
import { Badge } from "../../ui/badge/Badge";
import { PROTOTYPE_MOVE, PROTOTYPE_PROJECT } from "./data";

/** Settings › You › This machine › Pools, with `children` as the group's list. */
export function PrototypeSettingsFrame({ children }: { children: ReactNode }) {
  const [filter, setFilter] = useState("");
  return (
    <div className="proto-rsi-host">
      <SettingsLayout
        levels={[
          { id: "you", label: "You" },
          { id: "project", label: "Project" },
          { id: "workspace", label: "Workspace" },
        ]}
        level="you"
        onLevelChange={() => undefined}
        about="Your own settings, on this machine. Kept in your purlis folder, which only you see."
        groups={[
          { id: "you.look", label: "Look" },
          { id: "you.profiles", label: "Profiles" },
          { id: "you.machine", label: "This machine" },
          { id: "you.machine.pools", label: "Pools", sub: true },
          { id: "you.notifications", label: "Notifications" },
        ]}
        group="you.machine.pools"
        onGroupChange={() => undefined}
        filter={filter}
        onFilterChange={setFilter}
      >
        <SettingGroup
          label="Pools"
          help="The pools on this machine. Each runs a runner paired with this machine: a container of its image on your Docker, held to its limits, or a runner claimed in your team's cluster."
        >
          <SettingRow
            label="Pools on this machine"
            help="Remove takes this machine out of the runner's peers while it answers, then removes a Docker pool's container and home, or releases a cluster pool's claim in its cluster. There is no undo."
            grouped
            control={(ids) => (
              <div
                role="group"
                aria-labelledby={ids.labelledBy}
                aria-describedby={ids.describedBy}
                className="proto-rsi-list"
              >
                {children}
              </div>
            )}
          />
        </SettingGroup>
      </SettingsLayout>
    </div>
  );
}

/**
 * A stand-in of a chat's pane in project {@link PROTOTYPE_PROJECT}: its tab line, its row of
 * Notices (`notices`), and a few lines of its screen.
 */
export function PrototypeChatPane({ notices }: { notices: ReactNode }) {
  return (
    <div className="proto-rsi-host proto-rsi-pane" aria-label="A chat's pane">
      <div className="proto-rsi-pane-tab">
        <HarnessIcon kind="claude" />
        <strong>{PROTOTYPE_MOVE.chat}</strong>
        <span className="ui-setting-help">
          {PROTOTYPE_PROJECT} · profile {PROTOTYPE_MOVE.profile} · runs on this machine
        </span>
        <Badge variant="waiting">Move asked</Badge>
      </div>
      <div className="proto-rsi-pane-notices">{notices}</div>
      <pre className="proto-rsi-pane-screen" aria-label="The chat's screen">
        {[
          "⏺ Ran the upload tests 20 times: 3 failed on a timeout in upload_retry.",
          "⏺ The retry waits 50 ms against a server that takes up to 80 ms to answer.",
          "",
          "> Move this chat to pool team-gpu, the GPU runner is faster for the full suite",
        ].join("\n")}
      </pre>
    </div>
  );
}

/** A dialog drawn in place over the host page, on a scrim of its own. */
export function PrototypeScrim({ children }: { children: ReactNode }) {
  return (
    <div className="proto-rsi-scrim">
      <div className="proto-rsi-scrim-ground" aria-hidden="true" />
      <div className="proto-rsi-scrim-dialog">{children}</div>
    </div>
  );
}
