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
import { parse } from 'papaparse'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { ESideEffectType, RunSideEffectMixin } from '@/common/interceptors/run-side-effect.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { EntityWriteGuard } from '@/core/entity/entity-write.guard'
import { ImportCsvDto } from '@/core/entity/import-export/dto/import-csv.dto'
import { ImportJsonDto } from '@/core/entity/import-export/dto/import-json.dto'
import { ImportService } from '@/core/entity/import-export/import.service'
import {
  importCsvSchema,
  importJsonSchema
} from '@/core/entity/import-export/validation/schemas/import.schema'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'
import { CustomTransactionDecorator } from '@/database/neogma-dynamic/custom-transaction.decorator'
import { CustomTransactionInterceptor } from '@/database/neogma-dynamic/custom-transaction.interceptor'
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'

// ---------------------------------------------------------------------------------------------------------------------
// POST     /import/json           ✅ INGEST DATA
// POST     /import/csv            ✅ INGEST DATA
// POST     /import/yml            ❌ INGEST DATA
// POST     /import/xml            ❌ INGEST DATA
// ---------------------------------------------------------------------------------------------------------------------

@Controller('')
@ApiTags('Records')
@UseInterceptors(
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  CustomTransactionInterceptor
)
export class ImportController {
  constructor(private readonly importService: ImportService) {}

  @Post('/records/import/json')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard, EntityWriteGuard, CustomDbWriteRestrictionGuard)
  @UseInterceptors(
    RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]),
    TransformResponseInterceptor
  )
  @UsePipes(ValidationPipe(importJsonSchema, 'body'))
  @HttpCode(HttpStatus.CREATED)
  @AuthGuard('project')
  async collectJson(
    @Body() body: ImportJsonDto,
    @TransactionDecorator() transaction: Transaction,
    @CustomTransactionDecorator() customTx: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    const projectId = request.projectId

    return await this.importService.importRecords(body, projectId, transaction, customTx)
  }

  @Post('/records/import/csv')
  @ApiBearerAuth()
  @AuthGuard('project')
  @UseInterceptors(
    RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]),
    TransformResponseInterceptor
  )
  @UsePipes(ValidationPipe(importCsvSchema, 'body'))
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(PlanLimitsGuard, EntityWriteGuard, CustomDbWriteRestrictionGuard)
  async collectCsv(
    @Body() body: ImportCsvDto,
    @TransactionDecorator() transaction: Transaction,
    @CustomTransactionDecorator() customTx: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    const projectId = request.projectId

    const result = parse(body.payload, {
      header: true,
      dynamicTyping: body?.options?.suggestTypes ?? false,
      delimiter: ','
    })

    return await this.importService.importRecords(
      {
        payload: result.data,
        options: body.options,
        label: body.label
      },
      projectId,
      transaction,
      customTx
    )
  }
}
