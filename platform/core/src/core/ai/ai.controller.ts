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

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { AiService } from '@/core/ai/ai.service'
import { SchemaItem } from '@/core/ai/ai.types'
import { CreateEmbeddingIndexDto } from '@/core/ai/dto/create-embedding-index.dto'
import { GenerateSearchQueryDto } from '@/core/ai/dto/generate-search-query.dto'
import { SemanticSearchDto } from '@/core/ai/dto/semantic-search.dto'
import { UpsertIndexVectorsDto } from '@/core/ai/dto/upsert-index-vectors.dto'
import { SearchQueryGeneratorService } from '@/core/ai/search-query-generator.service'
import { createEmbeddingIndexSchema } from '@/core/ai/validation/schemas/embedding-index.schema'
import { TokenReadAccess } from '@/dashboard/auth/decorators/token-read-access.decorator'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { PlanActiveGuard } from '@/dashboard/billing/guards/plan-active.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'
import { releaseRequestTransaction } from '@/database/release-request-transaction'

class SchemaFilterDto {
  labels?: string[]
  /** When true, bypasses the 1-hour schema cache and forces a fresh recalculation. */
  force?: boolean
}

@Controller('ai')
@ApiTags('AI')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class AiController {
  constructor(
    private readonly aiService: AiService,
    private readonly searchQueryGeneratorService: SearchQueryGeneratorService
  ) {}

  /**
   * Returns the full graph schema as structured JSON.
   * Each item contains the label name, record count, properties with value ranges/samples,
   * and cross-label relationships.
   * Property `id` fields can be passed to /properties/:id/values for deeper drill-down.
   */
  @Post('/schema')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard)
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
  async getSchema(@Body() body: SchemaFilterDto, @Request() request: PlatformRequest): Promise<SchemaItem[]> {
    const raw: any = (request as any).raw ?? request
    // Schema reads/recalculations run on their own sessions; the idle request
    // transaction would only tick down its time budget while a recalculation runs.
    await releaseRequestTransaction(request)
    return this.aiService.getSchema({
      projectId: request.projectId,
      workspaceId: !raw.project?.customDb ? request.workspaceId : undefined,
      labels: body?.labels,
      force: body?.force
    })
  }

  /**
   * Returns the full graph schema as compact Markdown tables.
   * Intended for direct LLM consumption — token-efficient alternative to the JSON endpoint.
   */
  @Post('/schema/md')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard)
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
  async getSchemaMarkdown(
    @Body() body: SchemaFilterDto,
    @Request() request: PlatformRequest
  ): Promise<string> {
    const raw: any = (request as any).raw ?? request
    await releaseRequestTransaction(request)
    const schema = await this.aiService.getSchema({
      projectId: request.projectId,
      workspaceId: !raw.project?.customDb ? request.workspaceId : undefined,
      labels: body?.labels,
      force: body?.force
    })
    return this.aiService.buildMdSchema(schema)
  }

  @Post('/search-query')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard)
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
  async generateSearchQuery(@Body() body: GenerateSearchQueryDto, @Request() request: PlatformRequest) {
    const raw: any = (request as any).raw ?? request
    // The LLM round-trip(s) can take longer than the transaction time budget; the
    // request transaction is unused here (schema comes from cache / own sessions),
    // so release it before the long wait instead of letting it expire while idle.
    await releaseRequestTransaction(request)
    return this.searchQueryGeneratorService.generate({
      prompt: body?.prompt,
      currentQuery: body?.currentQuery,
      projectId: request.projectId,
      workspaceId: !raw.project?.customDb ? request.workspaceId : undefined
    })
  }

  /**
   * Lists all embedding index policies for this project.
   */
  @Get('/indexes')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
  async getIndexes(@Request() request: PlatformRequest) {
    return this.aiService.findIndexes(request.projectId)
  }

  /**
   * Creates a new embedding index policy for a string property.
   * The property must exist in Neo4j and must have type 'string'.
   */
  @Post('/indexes')
  @ApiBearerAuth()
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
   * Embedding indexes live in Postgres (not Neo4j); ownership is enforced inside
   * AiService.deleteIndex via row.projectId === projectId.
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
   * Ownership is enforced inside AiService like deleteIndex.
   */
  @Get('/indexes/:id/stats')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
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
  @UseGuards(PlanActiveGuard)
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
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
