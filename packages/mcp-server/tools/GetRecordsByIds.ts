// Copyright Collect Software, Inc.
// Licensed under the Apache License, Version 2.0

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
