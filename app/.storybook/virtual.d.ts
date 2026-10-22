// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The built-in themes as the core hands them to a window (`handed.plugin.ts`). */
declare module "virtual:purlis-handed" {
  const handed: { built_in: unknown[] };
  export default handed;
  /** Every built-in theme by its id, and each of its appearances by its id. */
  export const themes: Record<string, Record<string, unknown>>;
}

/** The bytes of each face the built-in themes name, as base64, by digest (`handed.plugin.ts`). */
declare module "virtual:purlis-faces" {
  const faces: Record<string, string>;
  export default faces;
}
