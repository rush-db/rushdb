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

export async function DetachRelation(params: {
  sourceId: string
  targetId?: string
  targetIds?: string[]
  relationType?: string
  direction?: 'outgoing' | 'incoming' | 'bidirectional'
  transactionId?: string
}) {
  const { sourceId, targetId, targetIds, relationType, direction = 'outgoing', transactionId } = params

  const options: any = {}
  if (relationType) {
    options.typeOrTypes = relationType
  }
  if (direction) {
    options.direction = direction
  }

  const targets: string[] =
    targetIds && targetIds.length > 0 ? targetIds
    : targetId ? [targetId]
    : []
  if (targets.length === 0) {
    return { success: false, message: 'No targetId(s) provided' }
  }

  await db.records.detach({ source: sourceId, target: targets, options }, transactionId)

  return {
    success: true,
    message: `Relationship detached from '${sourceId}' to ${targets.length} target record(s)`
  }
}
