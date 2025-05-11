import { Body, Controller, HttpCode, HttpStatus, Post, Request, UseInterceptors } from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'
import { unparse } from 'papaparse'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { ExportService } from '@/core/entity/import-export/export.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'
import { CustomTransactionInterceptor } from '@/database/neogma-dynamic/custom-transaction.interceptor'

// ---------------------------------------------------------------------------------------------------------------------
// POST     /export/json           ✅ INGEST DATA
// POST     /export/csv            ✅ INGEST DATA
// POST     /export/yml            ❌ INGEST DATA
// POST     /export/xml            ❌ INGEST DATA
// ---------------------------------------------------------------------------------------------------------------------

@Controller('')
@ApiTags('Records')
@UseInterceptors(
  NotFoundInterceptor,
  TransformResponseInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  CustomTransactionInterceptor
)
export class ExportController {
  constructor(private readonly exportService: ExportService) {}

  @Post('/records/export/csv')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async exportCsv(
    @Body() searchParams: SearchDto = {},
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ) {
    const projectId = request.projectId

    const data = await this.exportService.exportRecords({
      projectId,
      searchParams,
      transaction
    })

    return { fileContent: unparse(data), dateTime: getCurrentISO() }
  }
}
