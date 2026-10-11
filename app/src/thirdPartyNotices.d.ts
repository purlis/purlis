// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * The third-party notices `vite build` generates from the locked dependency graph
 * (`tools/third-party-notices.mjs`, #1784). The dev server and vitest serve `generated: false`
 * and nothing else: only a build reads every package's licence text.
 */
declare module "virtual:purlis-third-party" {
  const notices: {
    generated: boolean;
    /** Each shipped crate and npm package, naming its licence texts by index into `texts`. */
    packages: {
      kind: "crate" | "npm";
      name: string;
      version: string;
      licence?: string;
      texts: number[];
      /** The licences whose standard text stands in, where the package ships no licence file. */
      standard?: string[];
      authors?: string[];
    }[];
    /** Each distinct licence text, once. */
    texts: string[];
  };
  export default notices;
}
