// Copyright Collect Software, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { db } from '../util/db.js'

/**
 * Returns the full graph schema as structured JSON.
 * Each item contains: label name, record count, properties (with id, name, type,
 * value ranges for numbers/datetimes, sample values for strings), cross-label
 * relationships with direction (in/out), and an optional `vectorIndexes` array per
 * property — non-empty when embedding indexes exist, signalling semantic search
 * eligibility.
 *
 * Pass `force: true` to bypass the 1-hour schema cache and force a fresh recalculation.
 * Use this when you need property `id` values to pass to PropertyValues for deeper drill-down.
 * For initial schema orientation, prefer GetSchemaMarkdown (token-efficient).
 */
export async function getSchema(params: { labels?: string[]; force?: boolean } = {}) {
  const result = await db.ai.getSchema(params)
  return result.data
}
