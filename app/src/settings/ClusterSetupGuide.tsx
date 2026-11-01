// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ExternalLink } from "../ReleaseNotes";

/** The guide, on GitHub: the docs site isn't published yet (ruling V38). */
export const CLUSTER_SETUP_GUIDE =
  "https://github.com/purlis/purlis/blob/main/docs/cluster-pool-setup.md";

/**
 * **Where a cluster with nothing on it goes next** (#4537): a line where a cluster pool is added
 * and in the pools settings, until Pool setup's cluster path (#4492, #4493) takes its place.
 */
export function ClusterSetupGuide() {
  return (
    <p className="ui-setting-help">
      Nothing's set up on this cluster yet?{" "}
      <ExternalLink href={CLUSTER_SETUP_GUIDE}>Here's how</ExternalLink>.
    </p>
  );
}
