import { BadRequestException, forwardRef, Inject, Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { EntityService } from '@/core/entity/entity.service'
import { EntityRepository } from '@/core/entity/model/entity.repository'
import { DeletePropertyDto } from '@/core/property/dto/delete-property.dto'
import { UpdatePropertyValueDto } from '@/core/property/dto/update-property-value.dto'
import { UpdatePropertyDto } from '@/core/property/dto/update-property.dto'
import { PropertyQueryService } from '@/core/property/property-query.service'
import { PROPERTY_TYPE_STRING } from '@/core/property/property.constants'
import { SearchDto } from '@/core/search/dto/search.dto'
import { TSearchSortDirection } from '@/core/search/search.types'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { PropertyDto } from './dto/property.dto'
import { TPropertyInstance, TPropertyProperties } from './model/property.interface'
import { PropertyRepository } from './model/property.repository'

@Injectable()
export class PropertyService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly propertyRepository: PropertyRepository,
    private readonly propertyQueryService: PropertyQueryService,
    @Inject(forwardRef(() => EntityRepository))
    private readonly entityRepository: EntityRepository,
    @Inject(forwardRef(() => EntityService))
    private readonly entityService: EntityService
  ) {}

  async getPropertyValues({
    propertyId,
    sort,
    pagination,
    transaction,
    queryRunner
  }: {
    propertyId: string
    sort?: TSearchSortDirection
    pagination?: Pick<SearchDto, 'skip' | 'limit'>
    transaction: Transaction
    queryRunner?: QueryRunner
  }) {
    const runner = queryRunner || this.neogmaService.createRunner()

    return await runner
      .run(
        this.propertyQueryService.getPropertyValues({
          paginationParams: pagination,
          sort
        }),
        {
          id: propertyId
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
    const runner = queryRunner || this.neogmaService.createRunner()

    return await runner
      .run(this.propertyQueryService.getProjectProperties(), { id: projectId }, transaction)
      .then((res) => res.records[0].get('properties'))
  }

  async find({
    projectId,
    transaction,
    searchParams,
    queryRunner
  }: {
    projectId: string
    transaction: Transaction
    searchParams?: SearchDto
    queryRunner?: QueryRunner
  }): Promise<TPropertyProperties[]> {
    const runner = queryRunner || this.neogmaService.createRunner()

    return await runner
      .run(this.propertyQueryService.getProjectProperties(), { id: projectId }, transaction)
      .then((res) => res.records[0].get('properties'))
  }

  async update(
    propertyInstance: TPropertyInstance,
    property: PropertyDto,
    transaction: Transaction
  ): Promise<TPropertyInstance> {
    propertyInstance.metadata = property.metadata ?? propertyInstance.metadata
    propertyInstance.name = property.name ?? propertyInstance.name
    propertyInstance.type = property.type ?? propertyInstance.type

    return propertyInstance.save({ session: transaction })
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
    const runner = queryRunner || this.neogmaService.createRunner()

    await runner.run(
      this.propertyQueryService.cleanUpAfterProcessing(),
      {
        projectId
      },
      transaction
    )
  }

  async deleteFields({
    propertyId,
    deleteParams,
    projectId,
    transaction
  }: {
    propertyId: string
    deleteParams: DeletePropertyDto
    projectId: string
    transaction: Transaction
  }): Promise<boolean> {
    const runner = this.neogmaService.createRunner()

    await runner.run(
      this.propertyQueryService.deleteField(deleteParams.entityIds),
      {
        target: propertyId,
        entityIds: deleteParams.entityIds
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

  async updateField({
    propertyId,
    updateParams,
    transaction
  }: {
    propertyId: string
    updateParams: UpdatePropertyValueDto
    transaction: Transaction
  }): Promise<boolean> {
    const runner = this.neogmaService.createRunner()

    await runner.run(
      this.propertyQueryService.updateField(updateParams.entityIds),
      {
        target: propertyId,
        entityIds: updateParams.entityIds,
        newValue: updateParams.newValue
      },
      transaction
    )

    return true
  }

  async updateFieldData({
    propertyId,
    updateParams,
    transaction,
    projectId
  }: {
    propertyId: string
    updateParams: UpdatePropertyDto
    transaction: Transaction
    projectId: string
  }): Promise<boolean> {
    const runner = this.neogmaService.createRunner()

    const currentNode = await this.propertyRepository.model.findOne({
      where: { id: propertyId },
      throwIfNotFound: true,
      session: transaction
    })

    const conflictNode = await this.propertyRepository.model.findOne({
      where: { name: updateParams.name, projectId },
      throwIfNotFound: false,
      session: transaction
    })

    const applyType = updateParams.type ?? currentNode.type
    const shouldSelectMergeStrategy = conflictNode && updateParams.name !== currentNode.name
    const hasTypeConflict = shouldSelectMergeStrategy && applyType !== conflictNode.type
    const hasSameType = !updateParams.type && currentNode.type === conflictNode.type

    const canResolveTypeConflict =
      shouldSelectMergeStrategy &&
      ((applyType === PROPERTY_TYPE_STRING && conflictNode.type === PROPERTY_TYPE_STRING) || hasSameType)

    if (updateParams.name && hasTypeConflict && !canResolveTypeConflict) {
      throw new BadRequestException(
        `Property "${currentNode.name}" could not be merged onto property "${updateParams.name}" as they have conflicting types. Try to convert type before merging.`
      )
    }

    if (shouldSelectMergeStrategy) {
      await runner.run(
        this.propertyQueryService.mergeFieldMeta(),
        {
          target: propertyId,
          conflictNodeId: conflictNode.id,
          newType: conflictNode.type
        },
        transaction
      )

      await runner.run(
        this.propertyQueryService.deleteField(),
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

    await runner.run(
      this.propertyQueryService.updateFieldNode(),
      {
        target: propertyId,
        newName: updateParams.name ?? null,
        newType: updateParams.type ?? null
      },
      transaction
    )

    return true
  }
}