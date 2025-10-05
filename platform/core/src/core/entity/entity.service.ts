import { forwardRef, Inject, Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { Where } from '@/core/common/types'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import {
  TEntityPropertiesNormalized,
  TRecordRelationsResponse,
  TRelationDirection
} from '@/core/entity/entity.types'
import { PropertyService } from '@/core/property/property.service'
import { TPropertyProperties } from '@/core/property/property.types'
import { AttachDto } from '@/core/relationships/dto/attach.dto'
import { DetachDto } from '@/core/relationships/dto/detach.dto'
import { SearchDto } from '@/core/search/dto/search.dto'

import { CreateEntityDto } from './dto/create-entity.dto'
import { EditEntityDto } from './dto/edit-entity.dto'

@Injectable()
export class EntityService {
  constructor(
    private readonly entityQueryService: EntityQueryService,
    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService
  ) {}

  async create({
    entity,
    projectId,
    transaction
  }: {
    entity: CreateEntityDto
    projectId: string
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized> {
    const query = this.entityQueryService.createRecord()
    const { properties, label, id }: CreateEntityDto & { id?: string } = entity
    const entityId = id ?? uuidv7()

    const record: CreateEntityDto & { id?: string; created: string } = {
      id: entityId,
      created: getCurrentISO(),
      label,
      properties
    }

    const result = await transaction.run(query, { record, projectId })
    return result.records[0]?.get('data')
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
    const query = this.entityQueryService.getEntity()

    const result = await transaction.run(query, { id, projectId })
    return result.records[0]?.get('data')
  }

  async edit({
    entity,
    entityId,
    projectId,
    transaction
  }: {
    entityId: string
    projectId: string
    entity: EditEntityDto
    transaction: Transaction
  }): Promise<TEntityPropertiesNormalized | undefined> {
    const query = this.entityQueryService.editRecord()

    const { properties, label } = entity

    const record: EditEntityDto & { id?: string } = {
      id: entityId,
      label,
      properties
    }

    await transaction.run(query, { record, projectId })

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
    const query = this.entityQueryService.deleteRecord()

    await transaction.run(query, { id, projectId })

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction
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
    const query = this.entityQueryService.findRecords({ id, searchQuery })

    const queryResponse = await transaction.run(query, {
      projectId
    })

    // if (isObject(searchQuery.groupBy)) {
    return queryResponse.records.reduce((acc, r) => {
      acc.push(r.get('records'))
      return acc
    }, [])
    // }

    // return (queryResponse.records[0]?.get('records') ?? []) as TEntityPropertiesNormalized[]
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
    const query = this.entityQueryService.getRecordsCount({ searchQuery, id })
    const result = await transaction.run(query, { projectId })
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
    const query = this.entityQueryService.getEntityLabels(searchQuery)
    const result = await transaction.run(query, { id, projectId })
    return result.records.reduce(
      (acc, record) => ({
        ...acc,
        [record.get('label')]: record.get('total')
      }),
      {}
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
    const query = this.entityQueryService.getEntityPropertiesKeys({ searchQuery, id })
    const [allProjectProperties, filteredFields] = await Promise.all([
      this.propertyService.getProjectProperties({
        projectId,
        transaction
      }),
      transaction
        .run(query, { projectId })
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
    const query = this.entityQueryService.getRecordRelations({ id, searchQuery, pagination })

    const result = await transaction.run(query, { projectId })
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
    const query = this.entityQueryService.getRecordRelationsCount({ id, searchQuery })

    const result = await transaction.run(query, { projectId })
    return result.records[0]?.get('total') ?? 0
  }

  async attach(
    entityId: string,
    attachDto: AttachDto,
    projectId: string,
    transaction: Transaction
  ): Promise<{ message: string }> {
    const { targetIds, type, direction } = attachDto
    const query = this.entityQueryService.createRelation(type, direction)
    await transaction.run(query, {
      id: entityId,
      targetIds: isArray(targetIds) ? targetIds : [targetIds],
      projectId
    })

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
    const { targetIds, typeOrTypes, direction } = detachDto
    const query = this.entityQueryService.deleteRelations(typeOrTypes, direction)
    await transaction.run(query, {
      id: entityId,
      targetIds: isArray(targetIds) ? targetIds : [targetIds],
      projectId
    })

    return {
      message: `Relations to Record ${entityId} have been successfully deleted`
    }
  }

  async createRelationsByKeys({
    source,
    target,
    type,
    direction,
    projectId,
    transaction,
    manyToMany
  }: {
    source: { label: string; key?: string; where?: Where }
    target: { label: string; key?: string; where?: Where }
    type?: string
    direction?: TRelationDirection
    projectId: string
    transaction: Transaction
    manyToMany?: boolean
  }): Promise<void> {
    const query = this.entityQueryService.createRelationsByKeys({
      sourceLabel: source.label,
      sourceKey: source.key,
      targetLabel: target.label,
      targetKey: target.key,
      relationType: type,
      direction: direction === 'in' ? 'in' : 'out',
      sourceWhere: source.where,
      targetWhere: target.where,
      manyToMany
    })

    await transaction.run(query, { projectId })
  }

  async deleteRelationsByKeys({
    source,
    target,
    type,
    direction,
    projectId,
    transaction,
    manyToMany
  }: {
    source: { label: string; key?: string; where?: Where }
    target: { label: string; key?: string; where?: Where }
    type?: string
    direction?: TRelationDirection
    projectId: string
    transaction: Transaction
    manyToMany?: boolean
  }): Promise<void> {
    const query = this.entityQueryService.deleteRelationsByKeys({
      sourceLabel: source.label,
      sourceKey: source.key,
      targetLabel: target.label,
      targetKey: target.key,
      relationType: type,
      direction: direction === 'in' ? 'in' : 'out',
      sourceWhere: source.where,
      targetWhere: target.where,
      manyToMany
    })

    await transaction.run(query, { projectId })
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
    const query = this.entityQueryService.deleteRecords(searchQuery)

    await transaction.run(query, { id, projectId })

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction
    })

    return {
      message: `Records were successfully deleted`
    }
  }
}
