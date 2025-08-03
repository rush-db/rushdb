import { Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'

import { DeletePropertyDto } from '@/core/property/dto/delete-property.dto'
import { PropertyQueryService } from '@/core/property/property-query.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { TSearchSortDirection } from '@/core/search/search.types'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

import { TPropertyProperties } from './model/property.interface'
import { PropertyRepository } from './model/property.repository'

@Injectable()
export class PropertyService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly propertyRepository: PropertyRepository,
    private readonly propertyQueryService: PropertyQueryService,
    private readonly compositeNeogmaService: CompositeNeogmaService
  ) {}

  async getPropertyValues({
    propertyId,
    searchQuery,
    transaction,
    projectId,
    queryRunner
  }: {
    propertyId: string
    searchQuery: SearchDto & { query?: string; orderBy?: TSearchSortDirection }
    transaction: Transaction
    projectId: string
    queryRunner?: QueryRunner
  }) {
    const runner = queryRunner || this.compositeNeogmaService.createRunner()

    return await runner
      .run(
        this.propertyQueryService.getPropertyValues({
          searchQuery
        }),
        {
          id: propertyId,
          projectId
        },
        transaction
      )
      .then((res) => res.records[0]?.get('result') ?? {})
  }

  async getProjectProperties({
    projectId,
    transaction,
    queryRunner
  }: {
    projectId: string
    transaction: Transaction
    queryRunner?: QueryRunner
  }): Promise<TPropertyProperties[]> {
    const runner = queryRunner || this.compositeNeogmaService.createRunner()

    return await runner
      .run(this.propertyQueryService.getProjectProperties(), { id: projectId }, transaction)
      .then((res) => res.records[0].get('properties'))
  }

  async deleteOrphanProps({
    projectId,
    transaction,
    queryRunner
  }: {
    projectId: string
    transaction: Transaction
    queryRunner?: QueryRunner
  }): Promise<void> {
    const runner = queryRunner || this.compositeNeogmaService.createRunner()

    await runner.run(
      this.propertyQueryService.cleanUpAfterProcessing(),
      {
        projectId
      },
      transaction
    )
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
    const runner = this.compositeNeogmaService.createRunner()

    await runner.run(
      this.propertyQueryService.deletePropertyQuery(),
      {
        target: propertyId
      },
      transaction
    )

    await this.deleteOrphanProps({
      projectId,
      transaction,
      queryRunner: runner
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
      const property = await this.propertyRepository.model.findOne({
        where: { id: propertyId, projectId },
        throwIfNotFound: true,
        session: transaction
      })

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
