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
 * Generic label discovery / filtering tool.
 * Mirrors FindProperties / FindRecords style but restricted to labels domain.
 * Accepts standard query shape subset: where, limit, skip, orderBy.
 * (Aggregation & groupBy are not supported for labels.)
 */
export async function FindLabels(
  params: {
    where?: Record<string, any>
    limit?: number
    skip?: number
    orderBy?: Record<string, 'asc' | 'desc'>
  } = {}
) {
  const { where, limit, skip, orderBy } = params

  const searchQuery: Record<string, any> = {}
  if (where) searchQuery.where = where
  if (typeof limit === 'number') searchQuery.limit = limit
  if (typeof skip === 'number') searchQuery.skip = skip
  if (orderBy && Object.keys(orderBy).length > 0) searchQuery.orderBy = orderBy

  const response = await db.labels.find(searchQuery)

  // Existing labels.find returns an object mapping label->count when empty query supplied.
  // When filters applied, SDK still returns .data as mapping; normalize to array.
  if (response?.success && response.data) {
    return Object.entries(response.data).map(([name, count]) => ({ name, count }))
  }
  return []
}
