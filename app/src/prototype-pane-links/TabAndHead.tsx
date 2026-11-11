// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#6211, branch prototype/pane-links). Never merges to main.
 *
 * #5820's naming: the chat's number on its tab's tooltip, first line, and in its pane head
 * after its name, both "#641". Drawn with the chat strip's own tab (`ui/tabs/TabStrip.tsx`) and
 * the window's own pane head (`LayoutPanes.PaneHead`), so what is new is only the number.
 */
import type { CSSProperties } from "react";
import { Bot } from "lucide-react";
import { ChatMark } from "../ui/chat-mark/ChatMark";
import { StripTab, TabStripRow, useTabIds, useTabStrip } from "../ui/tabs/TabStrip";
import { TabCell, TabCloser, tabPart, tabsPart } from "../ui/tabs/Tabs";
import { TIPS_BELOW } from "../ui/tooltip/Tooltip";
import { ChatPersonaState } from "../ChatPersonaState";
import { ChatsHere, fixedChats, nothingKnown } from "../chatState";
import { PaneHead } from "../LayoutPanes";
import { QuestionsHere } from "../QuestionsHere";
import "./prototype.css";

/** The open chats, as the strip draws them, each with its number. */
const TABS = [
  {
    session: 1,
    number: 641,
    name: "Signed commits #5607",
    state: "working" as const,
    more: ["Works on #5607"],
  },
  {
    session: 2,
    number: 652,
    name: "Grill #5820: chat links",
    state: "waiting" as const,
    more: ["Handed off from steward 1"],
  },
  {
    session: 3,
    number: 660,
    name: "Docs site: the setup page",
    state: "working" as const,
    more: [],
  },
];

/**
 * **A tab's tooltip, with the number first** (#5820): "Chat #641", then the lines the tooltip
 * already says (where it was handed off from, its work item, a split's panes, its running tasks).
 * Never empty now: every chat has a number.
 */
export const tabTipWithNumber = (number: number, more: readonly string[]) =>
  [`Chat #${number}`, ...more].join("\n");

export function TabAndHead({ front = 0 }: { front?: number }) {
  const idOf = useTabIds();
  const along = useTabStrip({ keys: TABS.map((one) => one.name), selected: TABS[front].name });
  const chat = TABS[front];
  return (
    <ChatsHere.Provider
      value={fixedChats({
        ...nothingKnown,
        bySession: { 1: "running", 2: "waiting", 3: "running" },
      })}
    >
      <QuestionsHere.Provider value={{ asks: [], with_asking_chat: [] }}>
        <div className="editor pl-strip" {...tabsPart("layout")}>
          <header className="bar" {...tabsPart("strip")} {...TIPS_BELOW}>
            <TabStripRow
              strip={along}
              name="Chats"
              ids={TABS.map((_, at) => idOf(at))}
              className="tabs"
              style={{ "--least": "88px" } as CSSProperties}
            >
              {TABS.map((one, at) => (
                <TabCell
                  key={one.name}
                  className="tab"
                  variant="chat"
                  selected={at === front}
                  first={at === 0}
                >
                  <StripTab
                    strip={along}
                    tabKey={one.name}
                    selected={at === front}
                    tip={tabTipWithNumber(one.number, one.more)}
                    type="button"
                    id={idOf(at)}
                  >
                    <ChatMark state={one.state} who={<Bot aria-hidden="true" />} />
                    <span className="tab-name" {...tabPart("label")}>
                      {one.name}
                    </span>
                  </StripTab>
                  <TabCloser
                    label={`Close ${one.name}`}
                    reachable={at === front}
                    onPress={() => undefined}
                  />
                </TabCell>
              ))}
            </TabStripRow>
          </header>
          <div className="pane-frame pane-headed pl-head-frame">
            <div className="pane-top">
              <PaneHead grip={false} doing={null}>
                <div className="pane-chips" data-chat={chat.name}>
                  <span className="pane-persona">
                    <ChatPersonaState
                      persona="steward"
                      session={chat.session}
                      shell={false}
                      report={null}
                      outcome={null}
                      harness="Claude Code"
                    />
                  </span>
                  <span className="pane-name">{chat.name}</span>
                  {/* The number after the name, quiet: what every dispatch line calls it. */}
                  <span className="pl-head-number" data-testid="head-number">
                    #{chat.number}
                  </span>
                  <span className="pl-head-where">Claude Code · smart-ide</span>
                </div>
              </PaneHead>
            </div>
          </div>
        </div>
      </QuestionsHere.Provider>
    </ChatsHere.Provider>
  );
}
