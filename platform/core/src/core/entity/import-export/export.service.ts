import { Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import {
  RUSHDB_INTERNALS_ALIASES,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import { EntityService } from '@/core/entity/entity.service'
import { SearchDto } from '@/core/search/dto/search.dto'

@Injectable()
export class ExportService {
  constructor(private readonly entityService: EntityService) {}

  async collectAllDataFromDB({
    id,
    projectId,
    searchParams,
    transaction,
    total
  }: {
    id: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
    total: number
  }): Promise<any[]> {
    const batchSize = 1000
    let skip = 0
    const allData: any[] = []

    while (skip < total) {
      const remainingRecords = total - skip
      const limit = remainingRecords < batchSize ? remainingRecords : batchSize

      const batchData = await this.entityService.findRecords({
        id,
        projectId,
        searchParams: {
          where: searchParams.where,
          orderBy: searchParams.orderBy,
          skip,
          limit
        },
        transaction
      })

      if (batchData.length === 0) {
        // No more data to fetch
        break
      }

      allData.push(
        ...batchData.map((record) =>
          Object.entries(record).reduce((acc, [key, value]) => {
            const propertyName = RUSHDB_INTERNALS_ALIASES[key] ?? key
            if (key !== RUSHDB_KEY_PROPERTIES_META && key !== RUSHDB_KEY_PROJECT_ID) {
              acc[propertyName] = value
            }

            return acc
          }, {})
        )
      )
      skip += limit
    }

    return allData
  }

  async exportRecords({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }) {
    const total = await this.entityService.getRecordsTotalCount({
      id,
      projectId,
      searchParams,
      transaction
    })

    return await this.collectAllDataFromDB({
      id,
      projectId,
      searchParams,
      transaction,
      total
    })
  }
}
