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

export async function FindRelationships(params: {
  where?: Record<string, any>
  limit?: number
  skip?: number
  orderBy?: Record<string, 'asc' | 'desc'>
}) {
  const { where, limit = 10, skip = 0, orderBy } = params

  const searchQuery: any = { limit, skip }
  if (where) searchQuery.where = where
  if (orderBy && Object.keys(orderBy).length > 0) searchQuery.orderBy = orderBy

  const result = await db.relationships.find(searchQuery)
  if (result.success && result.data) return result.data
  return []
}
