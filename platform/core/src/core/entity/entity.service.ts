import { forwardRef, Inject, Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isArray } from '@/common/utils/isArray'
import { LinkEntityDto } from '@/core/entity/dto/link-entity.dto'
import { UnlinkEntityDto } from '@/core/entity/dto/unlink-entity.dto'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { TRecordRelationsResponse } from '@/core/entity/entity.types'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
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

  async createEntity({
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

  async getEntity({
    id,
    transaction
  }: {
    id: string
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getEntity(),
      {
        id
      },
      transaction
    )
    return result.records[0]?.get('data')
  }

  async editEntity({
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

    return this.getEntity({ id: entityId, transaction })
  }

  async deleteEntity(id: string, projectId: string, transaction: Transaction): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    await queryRunner.run(
      this.entityQueryService.deleteRecord(),
      {
        id
      },
      transaction
    )

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction,
      queryRunner
    })

    return {
      message: `Record ${id} and all nested Records and Files were successfully deleted`
    }
  }

  async findRecords({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized[]> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const queryResponse = await queryRunner.run(
      this.entityQueryService.findRecords({ id, searchParams }),
      { projectId },
      transaction
    )

    return (queryResponse.records[0]?.get('records') ?? []) as TEntityPropertiesNormalized[]
  }

  async getRecordsTotalCount({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<number> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordsCount({ searchParams, id }),
      {
        projectId
      },
      transaction
    )

    return result.records[0]?.get('total') ?? 0
  }

  async getRecordLabels({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<Record<string, number>> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    return await queryRunner
      .run(
        this.entityQueryService.getEntityLabels(searchParams),
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

  async getEntityFields({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
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
        .run(
          this.entityQueryService.getEntityPropertiesKeys({ searchParams, id }),
          { projectId },
          transaction
        )
        .then((result) => result.records[0].get('fields')) as unknown as string[]
    ])
    return allProjectProperties.filter((property) => filteredFields.includes(property.name))
  }

  async getRecordRelations({
    id,
    searchParams,
    pagination,
    projectId,
    transaction
  }: {
    id?: string
    searchParams?: SearchDto
    pagination?: Pick<SearchDto, 'skip' | 'limit'>
    projectId: string
    transaction: Transaction
  }): Promise<TRecordRelationsResponse['data']> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordRelations({
        id,
        searchParams,
        pagination
      }),
      { projectId },
      transaction
    )

    return result.records.map((r) => r.get('relation'))
  }

  async getRecordRelationsCount({
    id,
    searchParams,
    projectId,
    transaction
  }: {
    id?: string
    searchParams?: SearchDto
    projectId: string
    transaction: Transaction
  }): Promise<TRecordRelationsResponse['total']> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordRelationsCount({
        id,
        searchParams
      }),
      { projectId },
      transaction
    )

    return result.records[0]?.get('total') ?? 0
  }

  async attachRecords(
    entityId: string,
    linkingOptions: LinkEntityDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()
    const { targetIds, type, direction } = linkingOptions

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

  async detachRecords(
    entityId: string,
    unlinkOptions: UnlinkEntityDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()
    const { targetIds, typeOrTypes, direction } = unlinkOptions
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

  async deleteRecords({
    id,
    projectId,
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<{ message: string }> {
    const queryRunner = this.compositeNeogmaService.createRunner()

    await queryRunner.run(
      this.entityQueryService.deleteRecords(searchParams),
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
      message: `Record were successfully deleted`
    }
  }
}
