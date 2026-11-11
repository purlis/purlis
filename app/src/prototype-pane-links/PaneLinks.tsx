// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#6211, branch prototype/pane-links). Never merges to main.
 *
 * Question: what do links in a pane's output look like and do (spec #6208), and what is the
 * card a closed chat opens (#5820)? Every surface the spec names, on one stand-in pane:
 *
 *   hover        the link underlined, a tooltip with its resolved target and what ⌘-click does
 *   ⌘-click      a chat focuses or shows, a closed chat opens its card, a file opens in an editor
 *                tab, a URL in the browser; a file outside the workspace and an OSC 8 link whose
 *                text is another address ask first
 *   right-click  Open link and Copy link above the terminal's own rows, under-lined with the target
 *   ⌘⇧L          Links in this pane…: the links on screen, newest first, to open or copy
 *
 * Built from app/src/ui's families on stand-in data. The pane is a static picture of a terminal,
 * not xterm: the look is the question here, and the spec's link provider is a build ticket's.
 */
import { createRef, useState, type ReactNode, type RefObject } from "react";
import { Bot, ExternalLink, FileText, MessageSquare } from "lucide-react";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow, ToggleButton } from "../ui/button/Button";
import { Fact, Facts } from "../ui/card/Facts";
import { Dialog } from "../ui/dialog/Dialog";
import { ListBox, ListOption } from "../ui/list/List";
import { ListFilter, noMatchSaid } from "../ui/list/ListFilter";
import { RowBelow, RowDescription, RowLabel, RowLead } from "../ui/list/Row";
import { ContextMenu } from "../ui/menu/ContextMenu";
import { Menu, MenuItem, MenuSection, MenuSeparator, MenuWords } from "../ui/menu/Menu";
import { HeldPopover } from "../ui/popover/Popover";
import { AnchoredTooltip, TooltipDetail } from "../ui/tooltip/Tooltip";
import {
  CLOSED_CHAT,
  KIND_SAID,
  LINKS,
  MOD,
  ROWS,
  THIS_CHAT,
  WORKSPACE_DIR,
  linksNewestFirst,
  type Link,
} from "./links";
import "./prototype.css";

/** What the prototype is showing, beyond the pane at rest: a story starts on one. */
export type Moment =
  | { at: "rest" }
  | { at: "hover"; link: string }
  | { at: "menu"; link: string | null }
  | { at: "list" }
  | { at: "card" }
  | { at: "ask-outside" }
  | { at: "ask-osc8" };

const KIND_ICON = {
  chat: <MessageSquare aria-hidden="true" />,
  file: <FileText aria-hidden="true" />,
  url: <ExternalLink aria-hidden="true" />,
};

/**
 * **The whole prototype**: a chat's pane holding sample output, the bar under it that says what
 * the last act did, and every surface the pane opens. `start` is the moment a story opens on.
 */
export function PaneLinks({ start = { at: "rest" } }: { start?: Moment }) {
  const [moment, setMoment] = useState<Moment>(start);
  const [said, setSaid] = useState("Nothing opened yet. Hover a link; ⌘-click it; right-click it.");
  const [overlay, setOverlay] = useState(false);
  // One element ref per link, made once: what a tooltip and the card stand beside.
  const [anchors] = useState(
    () =>
      new Map<string, RefObject<HTMLSpanElement | null>>(
        Object.keys(LINKS).map((id) => [id, createRef<HTMLSpanElement>()]),
      ),
  );
  const anchor = (id: string) => anchors.get(id) as RefObject<HTMLSpanElement | null>;
  const rest = () => setMoment({ at: "rest" });

  /** ⌘-click, Open link, or Enter in the Links list: what the core's open command answers. */
  const open = (link: Link) => {
    const opens = link.opens;
    if (opens.how === "card") return setMoment({ at: "card" });
    if (opens.how === "ask-outside") return setMoment({ at: "ask-outside" });
    if (opens.how === "ask-osc8") return setMoment({ at: "ask-osc8" });
    rest();
    if (opens.how === "browser") return setSaid(`Opened ${link.target} in your browser.`);
    setSaid(opens.where + ".");
  };
  const copy = (link: Link) => {
    rest();
    setSaid(`Copied ${link.copies}`);
  };

  const hovered = moment.at === "hover" ? moment.link : null;
  const cardFrom = LINKS.chat641;

  return (
    <div className="pl-page">
      <div className="pl-window">
        <PaneHeadLine />
        <TerminalMenuOver
          menuLink={moment.at === "menu" ? moment.link : undefined}
          onMenu={(link) => setMoment({ at: "menu", link })}
          onClosed={() => setMoment((now) => (now.at === "menu" ? { at: "rest" } : now))}
          onOpen={open}
          onCopy={copy}
          onList={() => setMoment({ at: "list" })}
          onTerminalRow={(row) => {
            rest();
            setSaid(`${row} (the terminal's own row).`);
          }}
        >
          <pre
            className="pl-terminal"
            data-overlay={overlay || undefined}
            // A picture of a terminal: xterm's own text is what a screen reader hears in the
            // real pane, and the keyboard reaches its links by the Links list and the menu.
            aria-label={`Terminal of chat #${THIS_CHAT.number}`}
            tabIndex={0}
            onKeyDown={(event) => {
              if (
                (event.metaKey || event.ctrlKey) &&
                event.shiftKey &&
                event.key.toLowerCase() === "l"
              ) {
                event.preventDefault();
                setMoment({ at: "list" });
              }
            }}
          >
            {ROWS.map((row, at) => (
              <span className="pl-row" key={at}>
                {row.map((run, part) => {
                  if (typeof run === "string") return <span key={part}>{run}</span>;
                  if ("faint" in run)
                    return (
                      <span key={part} className="pl-faint">
                        {run.faint}
                      </span>
                    );
                  if ("not" in run)
                    return (
                      <span
                        key={part}
                        className="pl-not"
                        data-why={run.not.why}
                      >
                        {run.not.text}
                      </span>
                    );
                  const link = LINKS[run.link];
                  return (
                    <span
                      key={part}
                      ref={anchor(link.id)}
                      className="pl-link"
                      data-link={link.id}
                      data-kind={link.kind}
                      data-osc8={link.osc8 || undefined}
                      data-hovered={
                        hovered === link.id ||
                        (moment.at === "menu" && moment.link === link.id) ||
                        undefined
                      }
                      onPointerEnter={() =>
                        setMoment((now) =>
                          now.at === "rest" || now.at === "hover"
                            ? { at: "hover", link: link.id }
                            : now,
                        )
                      }
                      onPointerLeave={() =>
                        setMoment((now) => (now.at === "hover" ? { at: "rest" } : now))
                      }
                      onClick={(event) => {
                        // A plain click selects, as in any terminal (#6208 decision 5).
                        if (!(event.metaKey || event.ctrlKey)) return;
                        event.preventDefault();
                        open(link);
                      }}
                    >
                      {link.text}
                    </span>
                  );
                })}
                {"\n"}
              </span>
            ))}
          </pre>
        </TerminalMenuOver>
        {hovered !== null && (
          <AnchoredTooltip anchor={anchor(hovered)}>
            <LinkTip link={LINKS[hovered]} />
          </AnchoredTooltip>
        )}
        <HeldPopover
          triggerRef={anchor(cardFrom.id)}
          isOpen={moment.at === "card"}
          onOpenChange={(isOpen) => !isOpen && rest()}
          aria-labelledby="pl-card-title"
          side="bottom"
          className="pl-card-popover"
        >
          <ClosedChatCard
            onRead={() => {
              rest();
              setSaid("Opened chat #641's saved output, read-only, in a view tab.");
            }}
            onReopen={() => {
              rest();
              setSaid("Reopened chat #641 on its conversation, in a new tab (as #668).");
            }}
          />
        </HeldPopover>
      </div>

      <div className="pl-bar" role="group" aria-label="Prototype">
        <ToggleButton size="sm" isSelected={overlay} onChange={setOverlay}>
          Show what links
        </ToggleButton>
        <Button size="sm" onPress={() => setMoment({ at: "list" })}>
          Links in this pane… ⇧⌘L
        </Button>
        <p role="status" className="pl-said">
          {said}
        </p>
      </div>

      {moment.at === "list" && <LinksInThisPane onClose={rest} onOpen={open} onCopy={copy} />}
      {moment.at === "ask-outside" && (
        <OutsideAsk
          link={LINKS.hosts}
          onClose={() => {
            rest();
            setSaid("Not opened.");
          }}
          onOpen={() => {
            rest();
            setSaid("Opened /etc/hosts at line 3 in an editor tab.");
          }}
          onCopy={() => copy(LINKS.hosts)}
        />
      )}
      {moment.at === "ask-osc8" && (
        <Osc8Ask
          link={LINKS.setup}
          onClose={() => {
            rest();
            setSaid("Not opened.");
          }}
          onOpen={() => {
            rest();
            setSaid(`Opened ${LINKS.setup.target} in your browser.`);
          }}
          onCopy={() => copy(LINKS.setup)}
        />
      )}
    </div>
  );
}

/** The chat's pane head, with its number after its name (#5820): drawn plainly, for context. */
function PaneHeadLine() {
  return (
    <div className="pl-head">
      <Bot aria-hidden="true" className="pl-head-icon" />
      <span className="pl-head-name">{THIS_CHAT.title}</span>
      <span className="pl-head-number">#{THIS_CHAT.number}</span>
      <span className="pl-head-where">Claude Code · {THIS_CHAT.workspace}</span>
    </div>
  );
}

/** **A link's tooltip**: where it goes, in full, then where that is and what ⌘-click does. */
export function LinkTip({ link }: { link: Link }) {
  return (
    <span className="pl-tip">
      <span className="pl-tip-target">{link.osc8 ? `Goes to ${link.target}` : link.target}</span>
      <TooltipDetail>{link.does}</TooltipDetail>
    </span>
  );
}

/**
 * **The terminal's right-click, with a link's rows** (#6208 decision 10, #1945): over a link,
 * *Open link* and *Copy link* come first, each saying its target under it; then the terminal's
 * own rows, then the pane's. Opened from the keyboard (Shift+F10, the menu key) there is no link
 * under a pointer, so the menu offers *Links in this pane…* in their place.
 */
function TerminalMenuOver({
  menuLink,
  onMenu,
  onClosed,
  onOpen,
  onCopy,
  onList,
  onTerminalRow,
  children,
}: {
  menuLink: string | null | undefined;
  onMenu: (link: string | null) => void;
  onClosed: () => void;
  onOpen: (link: Link) => void;
  onCopy: (link: Link) => void;
  onList: () => void;
  onTerminalRow: (row: string) => void;
  children: ReactNode;
}) {
  const over = menuLink ? LINKS[menuLink] : undefined;
  return (
    <ContextMenu
      onAsked={(event) => {
        const on = (event.target as Element).closest<HTMLElement>("[data-link]");
        onMenu(on?.dataset.link ?? null);
      }}
      onOpenChange={(isOpen) => !isOpen && onClosed()}
      menu={() => (
        <Menu
          className="item-menu"
          aria-label="Terminal"
          onAction={(key) => {
            if (key === "open" && over) return onOpen(over);
            if (key === "copy-link" && over) return onCopy(over);
            if (key === "links") return onList();
            onTerminalRow(String(key));
          }}
        >
          <MenuSection aria-label="Link">
            {over ? (
              <>
                <MenuItem id="open" textValue="Open link" keys={`${MOD}-click`}>
                  <MenuWords
                    said="Open link"
                    under={over.osc8 ? `Goes to ${over.target}` : over.target}
                  />
                </MenuItem>
                <MenuItem id="copy-link" textValue="Copy link">
                  <MenuWords said="Copy link" under={over.copies} />
                </MenuItem>
              </>
            ) : (
              <MenuItem id="links" textValue="Links in this pane…" keys="⇧⌘L">
                <MenuWords said="Links in this pane…" />
              </MenuItem>
            )}
          </MenuSection>
          <MenuSeparator />
          <MenuSection aria-label="Terminal">
            <MenuItem id="Copy" textValue="Copy" isDisabled keys="⌘C">
              <MenuWords said="Copy" />
            </MenuItem>
            <MenuItem id="Paste" textValue="Paste" keys="⌘V">
              <MenuWords said="Paste" />
            </MenuItem>
            <MenuItem id="Select all" textValue="Select all" keys="⌘A">
              <MenuWords said="Select all" />
            </MenuItem>
          </MenuSection>
          <MenuSeparator />
          <MenuSection aria-label="Pane">
            {over && (
              <MenuItem id="Links in this pane…" textValue="Links in this pane…" keys="⇧⌘L">
                <MenuWords said="Links in this pane…" />
              </MenuItem>
            )}
            <MenuItem id="Split right" textValue="Split right">
              <MenuWords said="Split right" />
            </MenuItem>
            <MenuItem id="Split down" textValue="Split down">
              <MenuWords said="Split down" />
            </MenuItem>
          </MenuSection>
        </Menu>
      )}
    >
      {children}
    </ContextMenu>
  );
}

/**
 * **Links in this pane…** (#6208 decision 10): a quick picker, as Use skill… is, of the links on
 * screen, newest first. Typing narrows it; Enter opens the link by the same rules as ⌘-click
 * (a file outside and an OSC 8 mismatch still ask); ⌘C copies its target.
 */
export function LinksInThisPane({
  onClose,
  onOpen,
  onCopy,
}: {
  onClose: () => void;
  onOpen: (link: Link) => void;
  onCopy: (link: Link) => void;
}) {
  const [query, setQuery] = useState("");
  const all = linksNewestFirst();
  const shown = all.filter((one) =>
    `${one.text} ${one.target}`.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const noun = { one: "link", many: "links" };
  return (
    <Dialog
      onClose={onClose}
      title="Links in this pane"
      subtitle={<>Chat #{THIS_CHAT.number} · on screen, newest first</>}
      summary={<>⏎ opens · {MOD}C copies its target</>}
      className="pl-links"
    >
      <ListFilter
        noun={noun}
        total={all.length}
        query={query}
        onQuery={setQuery}
        controls="pl-links-list"
      />
      <div
        id="pl-links-list"
        onKeyDown={(event) => {
          if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "c") return;
          const id = (event.target as Element).closest<HTMLElement>("[data-key]")?.dataset.key;
          const link = all.find((one) => one.id === id);
          if (link === undefined) return;
          event.preventDefault();
          onCopy(link);
        }}
      >
        {shown.length === 0 ? (
          <p className="honest">{noMatchSaid(noun, query)}</p>
        ) : (
          <ListBox
            aria-label="Links on screen"
            size="lg"
            onAction={(key) => {
              const link = all.find((one) => one.id === key);
              if (link !== undefined) onOpen(link);
            }}
          >
            {shown.map((link) => (
              <ListOption key={link.id} id={link.id} textValue={link.text} data-key={link.id}>
                <RowLead said={KIND_SAID[link.kind]}>{KIND_ICON[link.kind]}</RowLead>
                <RowLabel className="pl-mono">{link.text}</RowLabel>
                <RowDescription>
                  {KIND_SAID[link.kind]}
                  {link.osc8 ? " · marked by the program" : ""}
                </RowDescription>
                <RowBelow>
                  {link.osc8 ? `Goes to ${link.target}` : link.target}
                  {link.opens.how.startsWith("ask") ? " · asks first" : ""}
                </RowBelow>
              </ListOption>
            ))}
          </ListBox>
        )}
      </div>
    </Dialog>
  );
}

/** How a closed chat's outcome is said on its card. */
const OUTCOME = {
  done: { badge: "Finished", variant: "ok" as const, line: (at: string) => `finished ${at}` },
  blocked: {
    badge: "Blocked",
    variant: "waiting" as const,
    line: (at: string) => `stopped blocked ${at}`,
  },
  failed: { badge: "Failed", variant: "waiting" as const, line: (at: string) => `failed ${at}` },
  closed: { badge: "Closed", variant: "neutral" as const, line: (at: string) => `closed ${at}` },
};

/**
 * **A closed chat's card** (#5820): "#641 · Signed commits #5607, finished 11:08", who started
 * it, where it ran, its report, and *Read it* (its saved output, read-only) and *Reopen*. The
 * same card opens from a `chat 641` link and from the switcher's finished row.
 */
export function ClosedChatCard({
  chat = CLOSED_CHAT,
  onRead,
  onReopen,
}: {
  chat?: typeof CLOSED_CHAT;
  onRead: () => void;
  onReopen: () => void;
}) {
  const outcome = OUTCOME[chat.outcome];
  return (
    <div className="pl-card">
      <div className="pl-card-head">
        <h3 id="pl-card-title" className="pl-card-title">
          #{chat.number} · {chat.title}
        </h3>
        <Badge variant={outcome.variant}>{outcome.badge}</Badge>
      </div>
      <p className="pl-card-when">
        {outcome.badge} at {chat.finished} · it is in no list of open chats now
      </p>
      <Facts aria-label={`About chat #${chat.number}`}>
        <Fact label="Started by">{chat.startedBy}</Fact>
        <Fact label="Ran in">{chat.ran}</Fact>
        <Fact label="Report">
          {chat.report === "" ? (
            <span className="ui-text-secondary">It sent no report.</span>
          ) : (
            <span className="pl-card-report">{chat.report}</span>
          )}
        </Fact>
      </Facts>
      <ButtonRow>
        <Button variant="primary" onPress={onRead}>
          Read it
        </Button>
        <Button onPress={onReopen}>Reopen</Button>
      </ButtonRow>
    </div>
  );
}

/** **The ask before a file outside the workspace opens** (#6208, the open command's `Ask`). */
export function OutsideAsk({
  link,
  onClose,
  onOpen,
  onCopy,
}: {
  link: Link;
  onClose: () => void;
  onOpen: () => void;
  onCopy: () => void;
}) {
  return (
    <Dialog
      onClose={onClose}
      title="Open a file outside this workspace?"
      subtitle={
        <>
          From chat #{THIS_CHAT.number} · {THIS_CHAT.title}
        </>
      }
      description={<>This chat's output names a file that is not in {THIS_CHAT.workspace}.</>}
      cancel={{ children: "Don't open" }}
      alternative={{ children: "Copy path", onPress: onCopy }}
      main={{ children: "Open file", onPress: onOpen }}
      initialFocus="main"
    >
      <Facts aria-label="The file">
        <Fact label="File">
          <span className="pl-mono">{link.target}</span>
        </Fact>
        <Fact label="Workspace">
          <span className="pl-mono">{WORKSPACE_DIR}</span>
        </Fact>
      </Facts>
      <p className="ui-text-secondary pl-ask-note">It opens in an editor tab. Nothing is run.</p>
    </Dialog>
  );
}

/** The host of an address, which the ask draws apart so a look-alike is seen. */
function hostOf(address: string): { before: string; host: string; after: string } {
  const match = /^(https?:\/\/)([^/?#]+)(.*)$/.exec(address);
  return match
    ? { before: match[1], host: match[2], after: match[3] }
    : { before: "", host: address, after: "" };
}

/**
 * **The ask for an OSC 8 link whose text is another address** (#6208): its text and where it
 * really goes, the real host drawn apart. The keyboard starts on Don't open, and Return never
 * opens it.
 */
export function Osc8Ask({
  link,
  onClose,
  onOpen,
  onCopy,
}: {
  link: Link;
  onClose: () => void;
  onOpen: () => void;
  onCopy: () => void;
}) {
  const real = hostOf(link.target);
  return (
    <Dialog
      onClose={onClose}
      title="This link goes somewhere its text doesn't say"
      subtitle={
        <>
          From chat #{THIS_CHAT.number} · {THIS_CHAT.title}
        </>
      }
      description="A program in this chat marked these words as a link to a different address."
      cancel={{ children: "Don't open" }}
      alternative={{ children: "Copy address", onPress: onCopy }}
      main={{ children: "Open in browser", onPress: onOpen, variant: "secondary", byReturn: false }}
      initialFocus="cancel"
    >
      <Facts aria-label="The link">
        <Fact label="Its text">
          <span className="pl-mono">{link.text}</span>
        </Fact>
        <Fact label="Goes to">
          <span className="pl-mono">
            {real.before}
            <strong className="pl-host">{real.host}</strong>
            {real.after}
          </span>
        </Fact>
      </Facts>
      <p className="ui-text-secondary pl-ask-note">Open it only if you expected {real.host}.</p>
    </Dialog>
  );
}
