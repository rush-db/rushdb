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

import { ensureInitialized } from '../util/db.js'

export async function FindProperty(params: { where?: Record<string, any>; limit?: number; skip?: number }) {
  const { where, limit, skip } = params
  const db = await ensureInitialized()

  const searchQuery: Record<string, any> = {}
  if (where) searchQuery.where = where
  if (limit) searchQuery.limit = limit
  if (skip) searchQuery.skip = skip

  const result = await db.properties.find(searchQuery)

  return result.data
}
