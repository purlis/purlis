// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { commands, SKILL_RUNS_PANEL, type SkillPanel, type SkillPanelKey } from "./bindings";
import type { Offer } from "./actions";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { AnsweredBlocks } from "./Views";

/**
 * **One skill's tab** (SK-3, #2766), opened from a skill run's chip or the Skill studio. Its
 * panels are studio panels (ADR 0101 §6), each answered in panel blocks by the core and drawn
 * here as any view's answer is: purlis's Runs panel first, which lists every run of the skill,
 * newest first, with what started it and its signals (tool calls, tool errors, refusals, asks,
 * tokens and time, never a pass or a fail), then each panel an extension adds.
 */
export function SkillTab({
  plane,
  name,
  run: wanted,
  changed,
  offerFor,
  onPress,
}: {
  plane: string;
  name: string;
  /** The run the tab was opened on, from a run's chip: marked in the Runs panel. */
  run?: string;
  changed: number;
  /** The catalogue, for the ways out purlis's own panels offer. */
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
}) {
  const [panels, setPanels] = useState<SkillPanel[]>();
  const [refused, setRefused] = useState<string>();
  useEffect(() => {
    let live = true;
    void commands.skillPanels(plane, name).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setPanels(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, name, changed]);

  return (
    <div className="skill-tab" data-testid="skill-tab">
      <h2 className="skill-tab-name">
        <code>{name}</code>
      </h2>
      {refused !== undefined ? (
        <EmptyState
          headline={`purlis could not read the panels of ${name}`}
          body={refused}
          testid="skill-panels-refused"
        />
      ) : panels === undefined ? (
        <p className="pending" aria-busy="true">
          <LoaderCircle className="node-icon spinning" />
          Reading the skill's panels…
        </p>
      ) : (
        panels.map((panel) => (
          <StudioPanelSection
            key={keyOf(panel.key)}
            panel={panel}
            name={name}
            current={wanted}
            offerFor={panel.key.kind === SKILL_RUNS_PANEL ? offerFor : undefined}
            onPress={panel.key.kind === SKILL_RUNS_PANEL ? onPress : undefined}
          />
        ))
      )}
    </div>
  );
}

/** What a panel is held by: `runs`, or `<extension>/<panel>` for one an extension adds. */
export function keyOf(key: SkillPanelKey): string {
  return key.kind === "extension" ? `${key.extension}/${key.panel}` : key.kind;
}

/** One panel on a skill's tab: its heading, who adds it, and its blocks or why it has none. An
 *  extension's panel is handed no catalogue: its blocks run no verb of purlis's. */
function StudioPanelSection({
  panel,
  name,
  current,
  offerFor,
  onPress,
}: {
  panel: SkillPanel;
  name: string;
  current?: string;
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
}) {
  const heading = useId();
  return (
    <section
      aria-labelledby={heading}
      data-testid={
        panel.key.kind === SKILL_RUNS_PANEL ? "skill-runs" : `skill-panel-${keyOf(panel.key)}`
      }
    >
      <h3 id={heading}>
        {panel.title}
        {panel.by !== null && <span className="quiet">{` · ${panel.by}`}</span>}
      </h3>
      {panel.refused !== null ? (
        <EmptyState
          variant="panel"
          headline={`${panel.title} could not be shown for ${name}`}
          body={panel.refused}
          testid="skill-panel-refused"
        />
      ) : (
        <AnsweredBlocks
          blocks={panel.blocks}
          label={panel.title}
          current={current}
          offerFor={offerFor}
          onPress={onPress}
        />
      )}
    </section>
  );
}
