import {
  Body,
  Controller,
  HttpCode,
  HttpStatus,
  Post,
  Put,
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
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

// ---------------------------------------------------------------------------------------------------------------------
// LABELS

// POST     /labels                      ✅ LABELS LIST SearchDto
// ---------------------------------------------------------------------------------------------------------------------

@Controller('labels')
@ApiTags('Labels')
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor
)
export class LabelsController {
  constructor(private readonly entityService: EntityService) {}

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async labelsSearch(
    @TransactionDecorator() transaction: Transaction,
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

  @Put()
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async updateLabels(
    @TransactionDecorator() transaction: Transaction,
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
}
