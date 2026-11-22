// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

export function flakyTitle(retries: number, test: string): string;
export function flakyAnnotation(flake: { retries: number; test: string; said?: string[] }): string;
