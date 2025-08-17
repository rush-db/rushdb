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
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'

@Controller('')
@ApiTags('Records')
@UseInterceptors(NotFoundInterceptor, DataInterceptor)
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
    @PreferredTransactionDecorator() transaction: Transaction,
    @PreferredTransactionDecorator() customTx: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    try {
      const projectId = request.projectId

      return await this.importService.importRecords(body, projectId, transaction, customTx)
    } catch (error) {
      console.error(error)
    }
  }

  @Post('/records/import/csv')
  @ApiBearerAuth()
  @UseInterceptors(
    RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]),
    TransformResponseInterceptor
  )
  @UsePipes(ValidationPipe(importCsvSchema, 'body'))
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(PlanLimitsGuard, EntityWriteGuard, CustomDbWriteRestrictionGuard)
  @AuthGuard('project')
  async collectCsv(
    @Body() body: ImportCsvDto,
    // To check limits after BFS parse (performed on default DB)
    @TransactionDecorator() transaction: Transaction,
    // Main tx to write data (could be default db or external)
    @PreferredTransactionDecorator() customTx: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    const projectId = request.projectId

    const result = parse(body.data, {
      header: true,
      dynamicTyping: body?.options?.suggestTypes ?? false,
      skipEmptyLines: true,
      delimiter: ','
    })
    const cleanedData = result.data.map((draft) => {
      const entries = Object.entries(draft).filter(([, value]) => value !== null)
      return Object.fromEntries(entries)
    })

    return await this.importService.importRecords(
      {
        data: cleanedData,
        options: body.options,
        label: body.label
      },
      projectId,
      transaction,
      customTx
    )
  }
}
