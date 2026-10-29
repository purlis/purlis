// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Devices** (#2052, spec #2041 stories 54 and 55): this machine and every runner it keeps, each
 * with its chats and browsers, its version, how it is and when it last answered; then each
 * pool's use per person. A view tab of its own, opened from the palette's Devices row, which is
 * offered only once this machine keeps a pool (story 6). It is the Runners view ADR 0089 §12
 * named (as amended 2026-10-27, for spec #2041), under the spec's name.
 *
 * - **The list family**: a section per device, its first row how it is, then a row per chat
 *   and per browser on it, and one for what it holds that this window has not open; and a
 *   section for what runs on a pool this machine no longer keeps. **No row acts yet**: opening a
 *   chat, a runner's shell or its log from its row is #3494's, and until then a row is a stop
 *   for the keyboard and the screen reader that reads it, and nothing more.
 * - **What Docker or the link said is drawn apart** from purlis's own words: after the row's
 *   facts, isolated (`<bdi>`), as said elsewhere. The core has already put it on one line, capped
 *   and with nothing a reader does not see.
 * - **The table family** for the use: one row per pool and person, with its hours this week (the
 *   ISO week in local time, #3492), and its money this week only where it declares a price per
 *   hour (#3491). With no pool priced there is no money column; a pool that declares none has an
 *   empty money cell. A cost is never guessed.
 * - **A chat's or browser's project** is named on its row once the window holds more than one
 *   (#3534), so two projects' chats of one name tell apart.
 * - **Refresh** reads every device again, as the view's coming on screen does. A runner still
 *   being asked is not asked twice: Refresh waits on that ask for it, and asks every other one.
 */
import { Fragment } from "react";
import { List, ListRow, ListRowSection } from "./ui/list/List";
import { RowDescription, RowLabel, RowTrail } from "./ui/list/Row";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
import { Notice } from "./Notice";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";
import type { DevicesShown, PlaneId } from "./bindings";
import {
  arranged,
  contactSaid,
  healthSaid,
  hoursSaid,
  moneySaid,
  rereadDevices,
  runningSaid,
  usageOf,
  useDevices,
  versionSaid,
  answerOf,
  type Asked,
  type DeviceRow,
} from "./devices";
import { DEVICES_TITLE } from "./tabs";
import { projectName } from "./StatusLine";

/**
 * **Where a row's chat is, where the window holds more than one project** (#3534): ` in <project>`,
 * so two projects' chats of one name tell apart; nothing with one project.
 */
type InProject = (plane: PlaneId) => string;

const inProjectOf =
  (projects: number): InProject =>
  (plane) =>
    projects > 1 ? ` in ${projectName(plane)}` : "";

/** `count` of `one`, said whole: `1 chat`, `2 chats`. */
const counted = (count: number, one: string, many: string) =>
  `${count} ${count === 1 ? one : many}`;

/** What a runner's row says of what it holds that this window has not open. */
function notOpenSaid(asked: Asked | undefined): string | undefined {
  const answer = answerOf(asked);
  if (answer === undefined) return undefined;
  const { chats, browsers } = answer.holds;
  const parts = [
    chats > 0 ? counted(chats, "chat", "chats") : undefined,
    browsers > 0 ? counted(browsers, "browser", "browsers") : undefined,
  ].filter((one) => one !== undefined);
  return parts.length === 0
    ? undefined
    : `${parts.join(" and ")} of projects not open in this window`;
}

/** A row's facts, then what was said elsewhere, apart and isolated. */
function Facts({ facts, said }: { facts: readonly string[]; said?: { by: string; text: string } }) {
  if (facts.length === 0 && said === undefined) return null;
  return (
    <RowDescription>
      {facts.join(" · ")}
      {said !== undefined && (
        <Fragment>
          {facts.length > 0 ? " · " : ""}
          {`${said.by}: `}
          <bdi>{said.text}</bdi>
        </Fragment>
      )}
    </RowDescription>
  );
}

/** A device's section: its name, how it is, and what runs on it. */
function DeviceSection({
  row,
  asked,
  version,
  now,
  inProject,
}: {
  row: DeviceRow;
  asked: Asked | undefined;
  version: string;
  now: Date;
  inProject: InProject;
}) {
  const runner = row.runner;
  const title =
    runner === undefined ? "This machine" : `Runner ${runner.runner} of pool ${runner.pool}`;
  const health =
    runner === undefined
      ? { badge: "here", line: "Where this window runs", said: undefined }
      : healthSaid(asked);
  const runs =
    runner === undefined
      ? versionSaid(version, version)
      : versionSaid(answerOf(asked)?.version ?? null, version);
  const facts = [
    runs,
    runner === undefined ? undefined : contactSaid(answerOf(asked)?.contacted ?? null, now),
    runner?.draining === true
      ? "An older runner: it keeps its chats until they end, then purlis removes it"
      : undefined,
  ].filter((one) => one !== undefined);
  const more = notOpenSaid(asked);
  const nothing = row.chats.length === 0 && row.browsers.length === 0 && more === undefined;
  return (
    <ListRowSection id={row.key} title={title} named={runner !== undefined}>
      <ListRow id={`${row.key}:standing`} textValue={`${title}: ${health.badge}`}>
        <RowLabel>{health.line}</RowLabel>
        <Facts facts={facts} said={health.said} />
        <RowTrail>
          <Badge variant="neutral">{health.badge}</Badge>
        </RowTrail>
      </ListRow>
      {row.chats.map((chat) => (
        <ListRow
          key={`${chat.plane}:${chat.session}`}
          id={`${row.key}:chat:${chat.plane}:${chat.session}`}
          textValue={`${chat.name}${inProject(chat.plane)}`}
        >
          <RowLabel>{chat.name}</RowLabel>
          <RowDescription>{`Chat${inProject(chat.plane)}`}</RowDescription>
        </ListRow>
      ))}
      {row.browsers.map((browser) => (
        <ListRow
          key={`${browser.plane}:${browser.program}`}
          id={`${row.key}:browser:${browser.plane}:${browser.program}`}
          textValue={`Browser ${browser.name} of ${browser.chat}${inProject(browser.plane)}`}
        >
          <RowLabel>{`Browser ${browser.name}`}</RowLabel>
          <RowDescription>{`Of ${browser.chat}${inProject(browser.plane)}`}</RowDescription>
        </ListRow>
      ))}
      {more !== undefined && (
        <ListRow id={`${row.key}:more`} textValue={more}>
          <RowLabel>{more}</RowLabel>
        </ListRow>
      )}
      {nothing && (
        <ListRow id={`${row.key}:nothing`} textValue="No chat or browser runs here">
          <RowLabel>No chat or browser runs here</RowLabel>
        </ListRow>
      )}
    </ListRowSection>
  );
}

/** What runs on a pool this machine no longer keeps, as a section of its own. */
function UnkeptSection({
  placement,
  inProject,
}: {
  placement: DevicesShown["placement"];
  inProject: InProject;
}) {
  if (placement.unkept.length === 0 && placement.unkept_browsers.length === 0) return null;
  return (
    <ListRowSection id="unkept" title="On a pool this machine no longer keeps">
      {placement.unkept.map((chat) => (
        <ListRow
          key={`${chat.plane}:${chat.session}`}
          id={`unkept:chat:${chat.plane}:${chat.session}`}
          textValue={`${chat.name}${inProject(chat.plane)}`}
        >
          <RowLabel>{chat.name}</RowLabel>
          <RowDescription>{`Chat${inProject(chat.plane)} · on pool ${chat.pool}`}</RowDescription>
        </ListRow>
      ))}
      {placement.unkept_browsers.map((browser) => (
        <ListRow
          key={`${browser.plane}:${browser.session}:${browser.name}`}
          id={`unkept:browser:${browser.plane}:${browser.session}:${browser.name}`}
          textValue={`Browser ${browser.name} of ${browser.chat}${inProject(browser.plane)}`}
        >
          <RowLabel>{`Browser ${browser.name}`}</RowLabel>
          <RowDescription>
            {`Of ${browser.chat}${inProject(browser.plane)} · on pool ${browser.pool}`}
          </RowDescription>
        </ListRow>
      ))}
    </ListRowSection>
  );
}

/** **The view tab**: the devices, then each pool's use. */
export function DevicesView({ now = new Date() }: { now?: Date }) {
  const { read, asked } = useDevices();
  if (read.state === "reading")
    return (
      <div className="devices-view" data-testid="devices-view">
        <p className="ui-setting-help">Reading this machine&apos;s devices…</p>
      </div>
    );
  if (read.state === "trouble")
    return (
      <div className="devices-view" data-testid="devices-view">
        <Notice
          cause="devices-not-read"
          tone="trouble"
          fixes={[{ label: "Refresh", onPress: () => void rereadDevices() }]}
        >
          {read.trouble}
        </Notice>
      </div>
    );
  const shown = read.shown;
  const rows = arranged(shown, asked);
  const use = usageOf(shown.pools, shown.placement.runners, asked, shown.owned);
  // Money only where a pool declares a price per hour (#3491): with none, no column for it.
  const priced = use.some((one) => one.price !== null);
  const inProject = inProjectOf(shown.projects);
  return (
    <div className="devices-view" data-testid="devices-view">
      <div className="devices-bar">
        <Button size="sm" onPress={() => void rereadDevices()}>
          Refresh
        </Button>
      </div>
      <List aria-label={DEVICES_TITLE} className="devices-list">
        {rows.map((row) => (
          <DeviceSection
            key={row.key}
            row={row}
            asked={asked[row.key]}
            version={shown.version}
            now={now}
            inProject={inProject}
          />
        ))}
        <UnkeptSection placement={shown.placement} inProject={inProject} />
      </List>
      {use.length > 0 && (
        <Table aria-label="Use per pool and person" className="devices-use">
          <TableHeader>
            <TableColumn isRowHeader>Pool</TableColumn>
            <TableColumn>Person</TableColumn>
            <TableColumn>Running</TableColumn>
            <TableColumn>Suspended</TableColumn>
            <TableColumn>CPUs requested</TableColumn>
            <TableColumn>Memory requested</TableColumn>
            <TableColumn>Hours this week</TableColumn>
            {priced && <TableColumn>Money this week</TableColumn>}
          </TableHeader>
          <TableBody>
            {use.map((one) => (
              <TableRow key={`${one.pool}:${one.person}`} id={`${one.pool}:${one.person}`}>
                <TableCell>{one.pool}</TableCell>
                <TableCell>{one.person}</TableCell>
                <TableCell>{runningSaid(one)}</TableCell>
                <TableCell>{`${one.suspended}`}</TableCell>
                <TableCell>{one.cpus}</TableCell>
                <TableCell>{`${one.memoryMib} MiB`}</TableCell>
                <TableCell>{hoursSaid(one)}</TableCell>
                {priced && <TableCell>{moneySaid(one) ?? ""}</TableCell>}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
