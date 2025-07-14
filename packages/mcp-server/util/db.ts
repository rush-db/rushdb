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

import 'dotenv/config'
import RushDB from '@rushdb/javascript-sdk'

const token = process.env.RUSHDB_API_KEY

if (!token) {
  throw new Error('RUSHDB_API_KEY environment variable is required')
}

export const db = new RushDB(token)

// Ensure RushDB is initialized
export const ensureInitialized = async () => {
  if (!RushDB.isInitialized()) {
    await db.waitForInitialization()
  }
  return db
}
