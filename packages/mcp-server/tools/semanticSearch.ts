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
import type { DBRecordInstance } from '@rushdb/javascript-sdk'

export async function semanticSearch(params: {
  propertyName: string
  query?: string
  queryVector?: number[]
  labels: string[]
  sourceType?: 'managed' | 'external'
  similarityFunction?: 'cosine' | 'euclidean'
  dimensions?: number
  where?: Record<string, unknown>
  topK?: number
  skip?: number
  limit?: number
}) {
  const result = await db.ai.search(params)
  return result.data.map((r: DBRecordInstance) => r.data)
}
