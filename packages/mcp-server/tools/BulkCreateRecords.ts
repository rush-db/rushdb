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
import { isFlatObject } from '../util/isFlatObject.js'

type BulkCreateRecordsArgs = {
  label: string
  data: Record<string, any>[]
  transactionId?: string
  options?: {
    mergeStrategy?: 'append' | 'rewrite'
    mergeBy?: string[]
    returnResult?: boolean
  }
}

type BulkCreateRecordsResult = {
  message: string
  ids: string[]
}

export async function BulkCreateRecords({
  label,
  data,
  transactionId,
  options
}: BulkCreateRecordsArgs): Promise<BulkCreateRecordsResult> {
  // Route to the correct SDK method based on data shape
  const allFlat = Array.isArray(data) && data.every(isFlatObject)
  const normalizedOptions = options ? { ...options } : {}

  // Ensure returnResult default true for MCP convenience if not specified
  if (normalizedOptions.returnResult === undefined) {
    normalizedOptions.returnResult = true
  }

  const result =
    allFlat ?
      await db.records.createMany({ label, data, options: normalizedOptions }, transactionId)
    : await db.records.importJson({ label, data, options: normalizedOptions }, transactionId)
  const ids = result.data.map((record: any) => record.id())

  return {
    message: `Successfully created ${ids.length} records with label '${label}'`,
    ids
  }
}
