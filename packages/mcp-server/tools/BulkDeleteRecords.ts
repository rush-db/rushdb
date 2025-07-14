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

type BulkDeleteRecordsArgs = {
  labels?: string[]
  where: Record<string, any>
}

type BulkDeleteRecordsResult = {
  message: string
}

export async function BulkDeleteRecords({
  labels,
  where
}: BulkDeleteRecordsArgs): Promise<BulkDeleteRecordsResult> {
  const db = await ensureInitialized()

  const searchQuery: any = { where }
  if (labels && labels.length > 0) {
    searchQuery.labels = labels
  }

  const result = await db.records.delete(searchQuery)

  return {
    message: result.data?.message || 'Records deleted successfully'
  }
}
