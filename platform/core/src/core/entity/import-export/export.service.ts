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
    searchQuery,
    transaction,
    total
  }: {
    id: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
    total: number
  }): Promise<any[]> {
    const batchSize = 1000
    let skip = 0
    const allData: any[] = []

    while (skip < total) {
      const remainingRecords = total - skip
      const limit = remainingRecords < batchSize ? remainingRecords : batchSize

      const batchData = await this.entityService.find({
        id,
        projectId,
        searchQuery: {
          where: searchQuery.where,
          orderBy: searchQuery.orderBy,
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
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }) {
    const total = await this.entityService.getCount({
      id,
      projectId,
      searchQuery,
      transaction
    })

    return await this.collectAllDataFromDB({
      id,
      projectId,
      searchQuery,
      transaction,
      total
    })
  }
}
