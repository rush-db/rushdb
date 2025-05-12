import { forwardRef, Inject, Injectable } from '@nestjs/common'
import Joi from 'joi'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isArray } from '@/common/utils/isArray'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { TRecordRelationsResponse } from '@/core/entity/entity.types'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
import { AttachDto } from '@/core/relationships/dto/attach.dto'
import { DetachDto } from '@/core/relationships/dto/detach.dto'
import { SearchDto } from '@/core/search/dto/search.dto'

import { CreateEntityDto } from './dto/create-entity.dto'
import { EditEntityDto } from './dto/edit-entity.dto'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'

@Injectable()
export class EntityService {
  constructor(
    private readonly compositeNeogmaService: CompositeNeogmaService,
    private readonly entityQueryService: EntityQueryService,
    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService
  ) {}

  async create({
    entity,
    projectId,
    transaction,
    queryRunner
  }: {
    entity: CreateEntityDto
    projectId: string
    transaction: Transaction
    queryRunner?: QueryRunner
  }): Promise<TEntityPropertiesNormalized> {
    const runner = queryRunner || this.compositeNeogmaService.createRunner()
    const { properties, label, id }: CreateEntityDto & { id?: string } = entity
    const entityId = id ?? uuidv7()

    const record: CreateEntityDto & { id?: string; created: string } = {
      id: entityId,
      created: getCurrentISO(),
      label,
      properties
    }

    return await runner
      .run(
        this.entityQueryService.createRecord(),
        {
          record,
          projectId
        },
        transaction
      )
      .then((result) => result.records[0]?.get('data'))
  }

  async getById({
    id,
    projectId,
    transaction
  }: {
    id: string
    projectId: string
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getEntity(),
      {
        id,
        projectId
      },
      transaction
    )
    return result.records[0]?.get('data')
  }

  async edit({
    entity,
    entityId,
    projectId,
    queryRunner,
    transaction
  }: {
    entityId: string
    projectId: string
    entity: EditEntityDto
    transaction: Transaction
    queryRunner?: QueryRunner
  }): Promise<TEntityPropertiesNormalized | undefined> {
    const runner = queryRunner || this.compositeNeogmaService.createRunner()

    const { properties, label } = entity

    const record: EditEntityDto & { id?: string } = {
      id: entityId,
      label,
      properties
    }

    await runner.run(
      this.entityQueryService.editRecord(),
      {
        record,
        projectId
      },
      transaction
    )

    return this.getById({ id: entityId, projectId, transaction })
  }

  async deleteById({
    id,
    projectId,
    transaction
  }: {
    id: string
    projectId: string
    transaction: Transaction
  }): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    await queryRunner.run(
      this.entityQueryService.deleteRecord(),
      {
        id,
        projectId
      },
      transaction
    )

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction,
      queryRunner
    })

    return {
      message: `Record ${id} was successfully deleted`
    }
  }

  async find({
    id,
    projectId,
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized[]> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const queryResponse = await queryRunner.run(
      this.entityQueryService.findRecords({ id, searchQuery }),
      { projectId },
      transaction
    )

    return (queryResponse.records[0]?.get('records') ?? []) as TEntityPropertiesNormalized[]
  }

  async getCount({
    id,
    projectId,
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }): Promise<number> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordsCount({ searchQuery, id }),
      {
        projectId
      },
      transaction
    )

    return result.records[0]?.get('total') ?? 0
  }

  async getLabels({
    id,
    projectId,
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }): Promise<Record<string, number>> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    return await queryRunner
      .run(
        this.entityQueryService.getEntityLabels(searchQuery),
        {
          id,
          projectId
        },
        transaction
      )
      .then(({ records }) =>
        records.reduce(
          (acc, record) => ({
            ...acc,
            [record.get('label')]: record.get('total')
          }),
          {}
        )
      )
  }

  async findProperties({
    id,
    projectId,
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }): Promise<TPropertyProperties[]> {
    const queryRunner = this.compositeNeogmaService.createRunner()
    const [allProjectProperties, filteredFields] = await Promise.all([
      this.propertyService.getProjectProperties({
        projectId,
        transaction,
        queryRunner
      }),
      queryRunner
        .run(this.entityQueryService.getEntityPropertiesKeys({ searchQuery, id }), { projectId }, transaction)
        .then((result) => result.records[0].get('fields')) as unknown as string[]
    ])
    return allProjectProperties.filter((property) => filteredFields.includes(property.name))
  }

  async findRelations({
    id,
    searchQuery,
    pagination,
    projectId,
    transaction
  }: {
    id?: string
    searchQuery?: SearchDto
    pagination?: Pick<SearchDto, 'skip' | 'limit'>
    projectId: string
    transaction: Transaction
  }): Promise<TRecordRelationsResponse['data']> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordRelations({
        id,
        searchQuery,
        pagination
      }),
      { projectId },
      transaction
    )

    return result.records.map((r) => r.get('relation'))
  }

  async findRelationsCount({
    id,
    searchQuery,
    projectId,
    transaction
  }: {
    id?: string
    searchQuery?: SearchDto
    projectId: string
    transaction: Transaction
  }): Promise<TRecordRelationsResponse['total']> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordRelationsCount({
        id,
        searchQuery
      }),
      { projectId },
      transaction
    )

    return result.records[0]?.get('total') ?? 0
  }

  async attach(
    entityId: string,
    attachDto: AttachDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()
    const { targetIds, type, direction } = attachDto

    await queryRunner.run(
      this.entityQueryService.createRelation(type, direction),
      {
        id: entityId,
        targetIds: isArray(targetIds) ? targetIds : [targetIds],
        projectId
      },
      transaction
    )

    return {
      message: `Relations to Record ${entityId} have been successfully created`
    }
  }

  async detach(
    entityId: string,
    detachDto: DetachDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()
    const { targetIds, typeOrTypes, direction } = detachDto
    await queryRunner.run(
      this.entityQueryService.deleteRelations(typeOrTypes, direction),
      {
        id: entityId,
        targetIds: isArray(targetIds) ? targetIds : [targetIds],
        projectId
      },
      transaction
    )

    return {
      message: `Relations to Record ${entityId} have been successfully deleted`
    }
  }

  async delete({
    id,
    projectId,
    searchQuery,
    transaction
  }: {
    id?: string
    projectId: string
    searchQuery?: SearchDto
    transaction: Transaction
  }): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    await queryRunner.run(
      this.entityQueryService.deleteRecords(searchQuery),
      {
        id,
        projectId
      },
      transaction
    )

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction,
      queryRunner
    })

    return {
      message: `Records were successfully deleted`
    }
  }
}
