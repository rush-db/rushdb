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
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { CustomDbAvailabilityGuard } from '@/dashboard/billing/guards/custom-db-availability.guard'
import { PlanActiveGuard } from '@/dashboard/billing/guards/plan-active.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'

@Controller('search')
@ApiTags('Search')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class SearchController {
  constructor(private readonly entityQueryService: EntityQueryService) {}

  @Post('/records-query')
  @ApiBearerAuth()
  @UseGuards(PlanActiveGuard, CustomDbAvailabilityGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async find(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: SearchDto,
    @Request() request: PlatformRequest
  ): Promise<string> {
    return this.entityQueryService.findRecords({ searchQuery })
  }
}
