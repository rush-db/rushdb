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

/**
 * Re-exports the per-request context primitives from db.ts.
 *
 * This module exists as a convenience alias so code that only needs context
 * management can import from a clearly named module without pulling in
 * the full db initialisation logic.
 */
export { requestContext, getDb, RequestContext } from './db.js'
