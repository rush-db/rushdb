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

export async function DetachRelation(params: {
  sourceId: string
  targetId: string
  relationType?: string
  direction?: 'outgoing' | 'incoming' | 'bidirectional'
}) {
  const { sourceId, targetId, relationType, direction = 'outgoing' } = params
  const db = await ensureInitialized()

  const options: any = {}
  if (relationType) {
    options.typeOrTypes = relationType
  }
  if (direction) {
    options.direction = direction
  }

  await db.records.detach({
    source: sourceId,
    target: targetId,
    options
  })

  return {
    success: true,
    message: `Relationship detached from '${sourceId}' to '${targetId}'`
  }
}
