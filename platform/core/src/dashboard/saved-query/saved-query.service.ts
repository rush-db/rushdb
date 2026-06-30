import { BadRequestException, Injectable, NotFoundException } from '@nestjs/common'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { ProjectService } from '@/dashboard/project/project.service'
import { CreateSavedQueryDto, UpdateSavedQueryDto } from '@/dashboard/saved-query/dto/create-saved-query.dto'
import { SavedQueryRepository } from '@/dashboard/saved-query/model/saved-query.repository'
import { PublicSavedQuery } from '@/dashboard/saved-query/saved-query.types'

import type { SavedQueryRow } from '@/database/sql/schema/types'

@Injectable()
export class SavedQueryService {
  constructor(
    private readonly savedQueryRepository: SavedQueryRepository,
    private readonly projectService: ProjectService
  ) {}

  async create(dto: CreateSavedQueryDto, projectId: string, createdBy?: string): Promise<PublicSavedQuery> {
    await this.projectService.getProject(projectId)

    if (dto.searchMode !== 'manual' && !dto.prompt?.trim()) {
      throw new BadRequestException('A prompt is required for AI and semantic saved queries')
    }

    const now = getCurrentISO()
    const row = await this.savedQueryRepository.create({
      id: uuidv7(),
      projectId,
      name: dto.name,
      searchMode: dto.searchMode,
      prompt: dto.prompt ?? null,
      searchQuery: JSON.stringify(dto.searchQuery ?? {}),
      semanticIndexId: dto.semanticIndexId ?? null,
      createdBy,
      createdAt: now,
      updatedAt: now
    })

    return this.toPublic(row)
  }

  async list(projectId: string): Promise<PublicSavedQuery[]> {
    const rows = await this.savedQueryRepository.findByProjectId(projectId)
    return rows.map((row) => this.toPublic(row))
  }

  async get(id: string, projectId: string): Promise<PublicSavedQuery> {
    return this.toPublic(await this.getOwned(id, projectId))
  }

  async update(id: string, projectId: string, dto: UpdateSavedQueryDto): Promise<PublicSavedQuery> {
    await this.getOwned(id, projectId)

    const patch: Partial<SavedQueryRow> = { updatedAt: getCurrentISO() }

    if (dto.name !== undefined) {
      patch.name = dto.name
    }
    if (dto.searchMode !== undefined) {
      patch.searchMode = dto.searchMode
    }
    if (dto.prompt !== undefined) {
      patch.prompt = dto.prompt
    }
    if (dto.searchQuery !== undefined) {
      patch.searchQuery = JSON.stringify(dto.searchQuery)
    }
    if (dto.semanticIndexId !== undefined) {
      patch.semanticIndexId = dto.semanticIndexId
    }

    const row = await this.savedQueryRepository.update(id, patch)
    return this.toPublic(row)
  }

  async delete(id: string, projectId: string): Promise<boolean> {
    await this.getOwned(id, projectId)
    return this.savedQueryRepository.delete(id)
  }

  private async getOwned(id: string, projectId: string): Promise<SavedQueryRow> {
    const row = await this.savedQueryRepository.findById(id)
    if (!row || row.projectId !== projectId) {
      throw new NotFoundException('Saved query not found')
    }
    return row
  }

  private toPublic(row: SavedQueryRow): PublicSavedQuery {
    return {
      ...row,
      searchQuery: this.parse(row.searchQuery)
    }
  }

  private parse(value: string): Record<string, unknown> {
    try {
      return JSON.parse(value)
    } catch {
      return {}
    }
  }
}
