/*
 * Copyright (c) 2024 Collect Software, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { db } from '../util/db.js'

export async function GetRecordsByIds(params: { recordIds: string[] }) {
  const { recordIds } = params
  if (!Array.isArray(recordIds) || recordIds.length === 0) {
    return { success: false, message: 'recordIds must be a non-empty array', data: [] }
  }

  const result = await db.records.findById(recordIds)
  return {
    success: true,
    count: result.data.length,
    data: result.data.map((r: any) => r.data)
  }
}
