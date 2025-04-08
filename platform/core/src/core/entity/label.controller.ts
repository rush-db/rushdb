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
import { SearchDto } from '@/core/search/dto/search.dto'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

import { EntityService } from './entity.service'

// ---------------------------------------------------------------------------------------------------------------------
// LABELS

// POST     /labels                      ✅ LABELS LIST SearchDto
// ---------------------------------------------------------------------------------------------------------------------

@Controller('labels')
@ApiTags('Records')
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor
)
export class LabelController {
  constructor(private readonly entityService: EntityService) {}

  @Post()
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async labelsSearch(
    @TransactionDecorator() transaction: Transaction,
    @Body() searchParams: Pick<SearchDto, 'where'>,
    @Request() request: PlatformRequest
  ): Promise<Record<string, number>> {
    const projectId = request.projectId

    return await this.entityService.getLabels({
      projectId,
      searchParams,
      transaction
    })
  }
}
