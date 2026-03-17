import {
  Body,
  Controller,
  HttpCode,
  HttpStatus,
  Post,
  Request,
  UseGuards,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { AiService } from '@/core/ai/ai.service'
import { OntologyItem } from '@/core/ai/ai.types'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

class OntologyFilterDto {
  labels?: string[]
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
  @UseGuards(IsRelatedToProjectGuard())
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
      transaction
    })
  }

  /**
   * Returns the full graph ontology as compact Markdown tables.
   * Intended for direct LLM consumption — token-efficient alternative to the JSON endpoint.
   */
  @Post('/ontology/md')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
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
      transaction
    })
    return this.aiService.buildMdSchema(ontology)
  }
}
