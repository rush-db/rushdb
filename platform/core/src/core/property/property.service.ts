import { Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import { DeletePropertyDto } from '@/core/property/dto/delete-property.dto'
import { PropertyQueryService } from '@/core/property/property-query.service'
import { TPropertyProperties } from '@/core/property/property.types'
import { SearchDto } from '@/core/search/dto/search.dto'
import { TSearchSortDirection } from '@/core/search/search.types'

@Injectable()
export class PropertyService {
  constructor(private readonly propertyQueryService: PropertyQueryService) {}

  async getPropertyValues({
    propertyId,
    searchQuery,
    transaction,
    projectId
  }: {
    propertyId: string
    searchQuery: SearchDto & { query?: string; orderBy?: TSearchSortDirection }
    transaction: Transaction
    projectId: string
  }) {
    return await transaction
      .run(
        this.propertyQueryService.getPropertyValues({
          searchQuery
        }),
        {
          id: propertyId,
          projectId
        }
      )
      .then((res) => res.records[0]?.get('result') ?? {})
  }

  async getProjectProperties({
    projectId,
    transaction
  }: {
    projectId: string
    transaction: Transaction
  }): Promise<TPropertyProperties[]> {
    return await transaction
      .run(this.propertyQueryService.getProjectProperties(), { id: projectId })
      .then((res) => res.records[0].get('properties'))
  }

  async deleteOrphanProps({
    projectId,
    transaction
  }: {
    projectId: string
    transaction: Transaction
  }): Promise<void> {
    await transaction.run(this.propertyQueryService.cleanUpAfterProcessing(), {
      projectId
    })
  }

  async deleteProperty({
    propertyId,
    deleteParams,
    projectId,
    transaction
  }: {
    propertyId: string
    deleteParams?: DeletePropertyDto
    projectId: string
    transaction: Transaction
  }): Promise<boolean> {
    await transaction.run(this.propertyQueryService.deletePropertyQuery(), {
      target: propertyId
    })

    await this.deleteOrphanProps({
      projectId,
      transaction
    })

    return true
  }

  async findById({
    propertyId,
    transaction,
    projectId
  }: {
    propertyId: string
    transaction: Transaction
    projectId: string
  }) {
    try {
      const property = await transaction
        .run(this.propertyQueryService.getPropertyQuery(), {
          propertyId,
          projectId
        })
        .then((res) => res.records[0].get('property')?.properties as TPropertyProperties)

      return {
        id: property.id,
        metadata: property.metadata,
        name: property.name,
        type: property.type
      }
    } catch {
      return undefined
    }
  }
}
