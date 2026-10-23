// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "./here";

import { commands, ENGINE_DOWNLOAD, type EngineDownloadShown } from "./bindings";
import { Notice } from "./Notice";

/**
 * **The browser engine's first download, shown as it goes** (#2207).
 *
 * The first browser on a machine downloads the pinned engine before it starts. The session host
 * says how far each download has come a few times a second at most, and the app keeps the latest
 * word of each until its start answers (`enginedownload.rs`). This draws one Notice per download
 * under way, wherever the person is, with **Cancel download**: nothing of a cancelled download is
 * kept, and its start is refused in words that say so.
 */
/** Whether two answers show the same downloads, saying the same. */
function sameDownloads(a: EngineDownloadShown[], b: EngineDownloadShown[]): boolean {
  return (
    a.length === b.length &&
    a.every((one, i) => one.program === b[i].program && one.says === b[i].says)
  );
}

export function EngineDownloadNotice() {
  const [shown, setShown] = useState<EngineDownloadShown[]>([]);
  const [cancelling, setCancelling] = useState<string | null>(null);
  const [why, setWhy] = useState<string | null>(null);

  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .engineDownloads()
        .then((answer) => {
          if (gone || answer.status !== "ok") return;
          // A window whose core answers nothing (a test's, an older build's) shows none, and an
          // answer that changes nothing renders nothing again.
          const now = Array.isArray(answer.data) ? answer.data : [];
          setShown((was) => (sameDownloads(was, now) ? was : now));
        })
        .catch(() => undefined);
    read();
    const listening = listen(ENGINE_DOWNLOAD, () => {
      if (!gone) read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  const cancel = (program: string) => {
    setCancelling(program);
    setWhy(null);
    void commands
      .cancelEngineDownload(program)
      .then((answer) => {
        if (answer.status !== "ok") setWhy(answer.error);
      })
      .catch(() => undefined)
      .finally(() => setCancelling(null));
  };

  return (
    <>
      {shown.map((download) => (
        <Notice
          key={download.program}
          cause={`engine-download:${download.program}`}
          fixes={[
            {
              label: "Cancel download",
              onPress: () => cancel(download.program),
              busy: cancelling === download.program,
            },
          ]}
        >
          {download.says}
          {why && cancelling === null && ` ${why}`}
        </Notice>
      ))}
    </>
  );
}
