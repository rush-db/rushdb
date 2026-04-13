import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Request,
  UseGuards,
  UseInterceptors,
  UsePipes
} from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { ValidationPipe } from '@/common/validation/validation.pipe'
import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { AiService } from '@/core/ai/ai.service'
import { OntologyItem } from '@/core/ai/ai.types'
import { CreateEmbeddingIndexDto } from '@/core/ai/dto/create-embedding-index.dto'
import { SemanticSearchDto } from '@/core/ai/dto/semantic-search.dto'
import { UpsertIndexVectorsDto } from '@/core/ai/dto/upsert-index-vectors.dto'
import { createEmbeddingIndexSchema } from '@/core/ai/validation/schemas/embedding-index.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { PlanActiveGuard } from '@/dashboard/billing/guards/plan-active.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

class OntologyFilterDto {
  labels?: string[]
  /** When true, bypasses the 1-hour ontology cache and forces a fresh recalculation. */
  force?: boolean
}

@Controller('ai')
@ApiTags('AI')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class AiController {
  constructor(private readonly aiService: AiService) {}

  /**
   * Returns the full graph ontology as structured JSON.
   * Each item contains the label name, record count, properties with value ranges/samples,
   * and cross-label relationships.
   * Property `id` fields can be passed to /properties/:id/values for deeper drill-down.
   */
  @Post('/ontology')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getOntology(
    @Body() body: OntologyFilterDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<OntologyItem[]> {
    const raw: any = (request as any).raw ?? request
    return this.aiService.getOntology({
      projectId: request.projectId,
      workspaceId: !raw.project?.customDb ? request.workspaceId : undefined,
      labels: body?.labels,
      force: body?.force,
      transaction
    })
  }

  /**
   * Returns the full graph ontology as compact Markdown tables.
   * Intended for direct LLM consumption — token-efficient alternative to the JSON endpoint.
   */
  @Post('/ontology/md')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getOntologyMarkdown(
    @Body() body: OntologyFilterDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<string> {
    const raw: any = (request as any).raw ?? request
    const ontology = await this.aiService.getOntology({
      projectId: request.projectId,
      workspaceId: !raw.project?.customDb ? request.workspaceId : undefined,
      labels: body?.labels,
      force: body?.force,
      transaction
    })
    return this.aiService.buildMdSchema(ontology)
  }

  /**
   * Lists all embedding index policies for this project.
   */
  @Get('/indexes')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getIndexes(@Request() request: PlatformRequest) {
    return this.aiService.findIndexes(request.projectId)
  }

  /**
   * Creates a new embedding index policy for a string property.
   * The property must exist in Neo4j and must have type 'string'.
   */
  @Post('/indexes')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(createEmbeddingIndexSchema, 'body'))
  @HttpCode(HttpStatus.CREATED)
  async createIndex(
    @Body() dto: CreateEmbeddingIndexDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ) {
    return this.aiService.createIndex(request.projectId, request.workspaceId, dto, transaction)
  }

  /**
   * Deletes an embedding index policy.
   * Note: IsRelatedToProjectGuard is intentionally omitted — embedding indexes live in
   * Postgres (not Neo4j) so their UUIDs cannot be verified via the Neo4j ownership check.
   * Ownership is enforced inside AiService.deleteIndex via row.projectId === projectId.
   */
  @Delete('/indexes/:id')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.ACCEPTED)
  async deleteIndex(@Param('id') id: string, @Request() request: PlatformRequest) {
    await this.aiService.deleteIndex(id, request.projectId)
    return { deleted: true, queued: true }
  }

  /**
   * Returns Neo4j-level statistics for an embedding index (total records vs indexed records).
   * Note: IsRelatedToProjectGuard omitted for the same reason as deleteIndex.
   */
  @Get('/indexes/:id/stats')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getIndexStats(
    @Param('id') id: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ) {
    return this.aiService.getIndexStats(request.projectId, id, transaction)
  }

  /**
   * Upserts BYOV vectors for records into the selected embedding index.
   */
  @Post('/indexes/:id/vectors/upsert')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async upsertIndexVectors(
    @Param('id') id: string,
    @Body() dto: UpsertIndexVectorsDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ) {
    return this.aiService.upsertIndexVectors(request.projectId, request.workspaceId, id, dto, transaction)
  }

  /**
   * Performs exact semantic search over records whose property has an embedding index.
   * The query text is embedded on the fly and matched by cosine similarity.
   */
  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(PlanActiveGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async semanticSearch(
    @Body() dto: SemanticSearchDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ) {
    const raw: any = (request as any).raw ?? request
    return this.aiService.semanticSearch(
      request.projectId,
      dto,
      transaction,
      !raw.project?.customDb ? request.workspaceId : undefined
    )
  }
}
