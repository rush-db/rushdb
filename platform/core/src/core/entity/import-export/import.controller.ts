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
import { BadRequestException } from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import { parse } from 'papaparse'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { ESideEffectType, RunSideEffectMixin } from '@/common/interceptors/run-side-effect.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { EntityWriteGuard } from '@/core/entity/entity-write.guard'
import { TEntityPropertiesNormalized } from '@/core/entity/entity.types'
import { ImportCsvDto } from '@/core/entity/import-export/dto/import-csv.dto'
import { ImportJsonDto } from '@/core/entity/import-export/dto/import-json.dto'
import { ImportService } from '@/core/entity/import-export/import.service'
import {
  importCsvSchema,
  importJsonSchema
} from '@/core/entity/import-export/validation/schemas/import.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'
import { TransactionDecorator } from '@/database/transaction.decorator'

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
    @TransactionDecorator() transaction: Transaction,
    @PreferredTransactionDecorator() customTx: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    try {
      const projectId = request.projectId
      return await this.importService.importRecords(body, projectId, transaction, customTx)
    } catch (error) {
      throw new BadRequestException('Import failed: ' + error.message, { cause: error })
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

    const defaultConfig = {
      header: true,
      delimiter: ',',
      skipEmptyLines: true,
      dynamicTyping: body?.options?.suggestTypes ?? false
    }
    const config = Object.assign({}, defaultConfig, body.parseConfig ?? {}, {
      // ensure dynamicTyping precedence from parseConfig if explicitly provided
      dynamicTyping:
        typeof body?.parseConfig?.dynamicTyping === 'boolean' ?
          body.parseConfig.dynamicTyping
        : defaultConfig.dynamicTyping
    })

    if (!body.label || !body.label.trim()) {
      throw new BadRequestException('`label` is required for CSV import')
    }

    const result = parse<Record<string, any>>(body.data, config as any) as any
    if (result.errors?.length) {
      throw new BadRequestException(`CSV parse error: ${result.errors[0].message}`)
    }

    // Sanitize and normalize headers (Neo4j token names cannot be empty or contain null-bytes)
    const originalFields: string[] = result?.meta?.fields || []
    const seen = new Set<string>()
    const fieldReplacements: Record<string, string> = {}
    const sanitizedFields = originalFields.map((raw, idx) => {
      let name = (raw ?? '').replace(/\0/g, '').trim()
      if (!name) {
        name = `column_${idx + 1}`
      }
      // Replace whitespace with underscore for stability
      name = name.replace(/\s+/g, '_')
      // Prevent starting with non-letter by prefixing underscore (optional safeguard)
      if (!/^[A-Za-z_]/.test(name)) {
        name = `_${name}`
      }
      // Deduplicate
      if (seen.has(name)) {
        const base = name
        let counter = 2
        while (seen.has(`${base}_${counter}`)) {
          counter++
        }
        name = `${base}_${counter}`
      }
      seen.add(name)
      fieldReplacements[raw] = name
      return name
    })

    // If we had headers but none after sanitation (unlikely) abort
    if (originalFields.length && !sanitizedFields.length) {
      throw new BadRequestException('CSV contains no usable headers')
    }

    // Transform each record with sanitized keys and drop null-only values
    const cleanedData = result.data.map((draft: Record<string, any>) => {
      if (draft && typeof draft === 'object' && !Array.isArray(draft)) {
        const entries = Object.entries(draft)
          .filter(([k, v]) => v !== null && v !== undefined && k !== '')
          .map(([k, v]) => [fieldReplacements[k] ?? k, v])
          // Filter again in case replacement produced an empty string (defensive)
          .filter(([k]) => k !== '')
        return Object.fromEntries(entries)
      }
      // If PapaParse returns arrays (header: false), convert to object with generated keys
      if (Array.isArray(draft)) {
        return draft.reduce((acc: Record<string, any>, val: any, i: number) => {
          acc[`column_${i + 1}`] = val
          return acc
        }, {})
      }
      return {}
    })

    // Final defensive check: ensure no empty property keys remain
    for (const row of cleanedData) {
      if (row && typeof row === 'object') {
        for (const key of Object.keys(row)) {
          if (!key || /\0/.test(key)) {
            throw new BadRequestException('CSV produced invalid empty property name after sanitation')
          }
        }
      }
    }

    if (!cleanedData.length) {
      return true
    }

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
