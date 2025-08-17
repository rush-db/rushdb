import {
  Body,
  Controller,
  HttpCode,
  HttpStatus,
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
import { EntityService } from '@/core/entity/entity.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'

@Controller('labels')
@ApiTags('Labels')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class LabelsController {
  constructor(private readonly entityService: EntityService) {}

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async labelsSearch(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: Pick<SearchDto, 'where'>,
    @Request() request: PlatformRequest
  ): Promise<Record<string, number>> {
    const projectId = request.projectId

    return await this.entityService.getLabels({
      projectId,
      searchQuery,
      transaction
    })
  }

  // @TODO: Add SearchQuery based PUT method to bulk update labels based on criteria
}
