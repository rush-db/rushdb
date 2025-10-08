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
import { CypherDto } from '@/core/query/dto/cypher.dto'
import { cypherSchema } from '@/core/query/validation/schemas/cypher.schema'
import { SearchDto } from '@/core/search/dto/search.dto'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { CustomDbAvailabilityGuard } from '@/dashboard/billing/guards/custom-db-availability.guard'
import { PlanActiveGuard } from '@/dashboard/billing/guards/plan-active.guard'
import { ExternalTransactionDecorator } from '@/database/external-transaction.decorator'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

@Controller('query')
@ApiTags('Query')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class QueryController {
  constructor(private readonly entityQueryService: EntityQueryService) {}

  @Post('/records/find')
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

  @Post('/raw')
  @ApiBearerAuth()
  @UseGuards(PlanActiveGuard, CustomDbAvailabilityGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(cypherSchema, 'body'))
  @UseInterceptors(DataInterceptor.withOptions(false, false, true))
  @HttpCode(HttpStatus.OK)
  async runCypher(@ExternalTransactionDecorator() transaction: Transaction, @Body() body: CypherDto) {
    const { query, params } = body
    const result = await transaction.run(query, params)

    // Convert neo4j Records to plain objects before returning. This ensures
    // consistent JSON serialization for clients (some driver internals are
    // non-enumerable and may be lost during generic serialization).
    const records =
      result.records?.map((r: any) => (typeof r.toObject === 'function' ? r.toObject() : null)) ?? []

    const resultSummary = result.summary

    const summary =
      resultSummary ?
        {
          query: resultSummary?.query?.text,
          resultAvailableAfter: resultSummary?.resultAvailableAfter,
          resultConsumedAfter: resultSummary?.resultConsumedAfter,
          counters: resultSummary?.counters
        }
      : undefined

    return {
      records,
      summary
    }
  }
}
