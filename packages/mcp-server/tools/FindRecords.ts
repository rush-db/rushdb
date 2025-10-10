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

export async function FindRecords(params: {
  labels?: string[]
  where?: Record<string, any>
  limit?: number
  skip?: number
  orderBy?: Record<string, 'asc' | 'desc'>
  aggregate?: Record<string, { fn: string; field?: string; alias?: string; where?: any }>
  groupBy?: string[]
}) {
  const { labels, where, limit = 10, skip = 0, orderBy, aggregate, groupBy } = params

  const searchQuery: any = {}
  if (labels && labels.length > 0) searchQuery.labels = labels
  if (where) searchQuery.where = where
  if (limit) searchQuery.limit = limit
  if (skip) searchQuery.skip = skip
  if (orderBy && Object.keys(orderBy).length > 0) searchQuery.orderBy = orderBy
  if (aggregate && Object.keys(aggregate).length > 0) searchQuery.aggregate = aggregate
  if (groupBy && groupBy.length > 0) searchQuery.groupBy = groupBy

  const result = await db.records.find(searchQuery)

  // If aggregation present, return raw response so caller gets aggregates.
  if (searchQuery.aggregate || searchQuery.groupBy) {
    return result
  }

  return result.data.map((record: any) => record.data)
}
