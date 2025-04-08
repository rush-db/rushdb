import { forwardRef, Inject, Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isArray } from '@/common/utils/isArray'
import { toBoolean } from '@/common/utils/toBolean'
import { LinkEntityDto } from '@/core/entity/dto/link-entity.dto'
import { UnlinkEntityDto } from '@/core/entity/dto/unlink-entity.dto'
import { UpsertEntityDto } from '@/core/entity/dto/upsert-entity.dto'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { TRecordRelationsResponse } from '@/core/entity/entity.types'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { CreateEntityDto, CreateEntityDtoSimple } from './dto/create-entity.dto'
import { EditEntityDto, EditEntityDtoSimple } from './dto/edit-entity.dto'

@Injectable()
export class EntityService {
  constructor(
    private readonly neogmaService: NeogmaService,
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
    const runner = queryRunner || this.neogmaService.createRunner()

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

  async upsert({
    entity,
    projectId,
    transaction,
    queryRunner
  }: {
    entity: UpsertEntityDto
    projectId: string
    transaction: Transaction
    queryRunner?: QueryRunner
  }): Promise<TEntityPropertiesNormalized> {
    const runner = queryRunner || this.neogmaService.createRunner()

    const { properties, label, id, matchBy }: UpsertEntityDto & { id?: string } = entity
    const entityId = id ?? uuidv7()

    let existingRecordsByCriteria: TEntityPropertiesNormalized[]
    if (toBoolean(matchBy)) {
      const where: SearchDto['where'] = Object.entries(entity.properties).reduce((acc, [key, value]) => {
        if (matchBy.includes(key)) {
          acc[key] = value.value
        }
        return acc
      }, {})

      existingRecordsByCriteria = await this.find({ searchParams: { where }, projectId, transaction })
    }

    const record: UpsertEntityDto & { id?: string; created: string } = {
      id: entityId,
      created: getCurrentISO(),
      label,
      properties
    }

    return await runner
      .run(
        this.entityQueryService.upsert(),
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
    const queryRunner = this.neogmaService.createRunner()

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
    const runner = queryRunner || this.neogmaService.createRunner()

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
    const queryRunner = this.neogmaService.createRunner()

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
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized[]> {
    const queryRunner = this.neogmaService.createRunner()

    const queryResponse = await queryRunner.run(
      this.entityQueryService.findRecords({ id, searchParams }),
      { projectId },
      transaction
    )

    return (queryResponse.records[0]?.get('records') ?? []) as TEntityPropertiesNormalized[]
  }

  async getCount({
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
    const queryRunner = this.neogmaService.createRunner()

    const result = await queryRunner.run(
      this.entityQueryService.getRecordsCount({ searchParams, id }),
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
    searchParams,
    transaction
  }: {
    id?: string
    projectId: string
    searchParams?: SearchDto
    transaction: Transaction
  }): Promise<Record<string, number>> {
    const queryRunner = this.neogmaService.createRunner()

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

  async findProperties({
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
    const queryRunner = this.neogmaService.createRunner()
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

  async findRelations({
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
    const queryRunner = this.neogmaService.createRunner()

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

  async findRelationsCount({
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
    const queryRunner = this.neogmaService.createRunner()

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

  async attach(
    entityId: string,
    linkingOptions: LinkEntityDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.neogmaService.createRunner()
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

  async detach(
    entityId: string,
    unlinkOptions: UnlinkEntityDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const queryRunner = this.neogmaService.createRunner()
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

  async delete({
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
    const queryRunner = this.neogmaService.createRunner()

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
      message: `Records were successfully deleted`
    }
  }
}
