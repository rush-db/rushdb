import { BadRequestException } from '@nestjs/common'
import {
  Body,
  Controller,
  Delete,
  Get,
  Headers,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Request,
  UseGuards,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiParam, ApiTags } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { formatErrorMessage } from '@/common/validation/utils'
import { EntityWriteGuard } from '@/core/entity/entity-write.guard'
import { TokenReadAccess } from '@/dashboard/auth/decorators/token-read-access.decorator'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'

import { ImportRunsService } from '../import-runs.service'

import { createImportRunSchema } from './validation/import-run.schema'

import type { ImportFileManifestItem, ImportLinkSpec } from '../domain/import-run.types'
import type { CreateImportRunDto } from './validation/import-run.schema'

@Controller('imports')
@ApiTags('Import Runs')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor)
export class ImportRunsController {
  constructor(private readonly importRunsService: ImportRunsService) {}

  @Post()
  @ApiBearerAuth()
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async create(
    @Request() request: PlatformRequest,
    @Body() body: unknown,
    @Headers('idempotency-key') idempotencyKey?: string
  ) {
    const projectId = request.projectId as string
    const parsed = createImportRunSchema.safeParse(body)
    if (!parsed.success) {
      throw new BadRequestException(formatErrorMessage(parsed.error, { type: 'body' }))
    }
    const manifest = parsed.data as CreateImportRunDto

    const result = await this.importRunsService.createDraft(
      {
        projectId,
        name: manifest.name,
        failurePolicy: manifest.failurePolicy,
        files: manifest.files.map(
          (file): ImportFileManifestItem => ({
            clientFileId: file.clientFileId,
            fileName: file.fileName,
            size: file.size,
            format: file.format,
            role: file.role,
            rootLabel: file.rootLabel,
            // Structural shape already validated by the schema; semantic
            // endpoint/source-target rules are enforced by the service layer.
            linkSpec: (file.linkSpec ?? undefined) as ImportLinkSpec | undefined,
            parseOptions: file.parseOptions,
            importOptions: file.importOptions
          })
        )
      },
      idempotencyKey ? this.importRunsService.hashKey(idempotencyKey) : undefined
    )

    return { runId: result.runId, files: result.files }
  }

  @Post(':runId/files/:fileId/content')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @ApiParam({ name: 'fileId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async uploadContent(
    @Request() request: PlatformRequest,
    @Param('runId') runId: string,
    @Param('fileId') fileId: string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    @Body() content: any
  ) {
    const projectId = request.projectId as string
    const buffer = Buffer.isBuffer(content) ? content : Buffer.from(JSON.stringify(content))

    return this.importRunsService.uploadFileContent(projectId, runId, fileId, buffer)
  }

  @Post(':runId/files/:fileId/upload/initiate')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @ApiParam({ name: 'fileId' })
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async initiateUpload(
    @Request() request: PlatformRequest,
    @Param('runId') runId: string,
    @Param('fileId') fileId: string
  ) {
    return this.importRunsService.initiateUpload(request.projectId as string, runId, fileId)
  }

  @Post(':runId/files/:fileId/upload/parts/:partNumber/sign')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @ApiParam({ name: 'fileId' })
  @ApiParam({ name: 'partNumber' })
  @HttpCode(HttpStatus.OK)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async signPart(
    @Request() request: PlatformRequest,
    @Param('runId') runId: string,
    @Param('fileId') fileId: string,
    @Param('partNumber') partNumber: string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    @Body() body: any
  ) {
    const uploadId = typeof body?.uploadId === 'string' ? body.uploadId : ''
    if (!uploadId) {
      throw new BadRequestException('uploadId is required')
    }
    return this.importRunsService.signPart(
      request.projectId as string,
      runId,
      fileId,
      uploadId,
      Number(partNumber)
    )
  }

  @Post(':runId/files/:fileId/upload/complete')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @ApiParam({ name: 'fileId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async completeUpload(
    @Request() request: PlatformRequest,
    @Param('runId') runId: string,
    @Param('fileId') fileId: string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    @Body() body: any
  ) {
    const uploadId = typeof body?.uploadId === 'string' ? body.uploadId : ''
    if (!uploadId) {
      throw new BadRequestException('uploadId is required')
    }
    const expectedSizeBytes =
      typeof body?.expectedSizeBytes === 'number' ? Math.floor(body.expectedSizeBytes) : undefined

    return this.importRunsService.completeUpload(
      request.projectId as string,
      runId,
      fileId,
      uploadId,
      expectedSizeBytes
    )
  }

  @Post(':runId/files/:fileId/upload/abort')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @ApiParam({ name: 'fileId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async abortUpload(
    @Request() request: PlatformRequest,
    @Param('runId') runId: string,
    @Param('fileId') fileId: string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    @Body() body: any
  ) {
    const uploadId = typeof body?.uploadId === 'string' ? body.uploadId : ''
    if (!uploadId) {
      throw new BadRequestException('uploadId is required')
    }
    await this.importRunsService.abortUpload(request.projectId as string, runId, fileId, uploadId)
    return { status: 'aborted' }
  }

  @Get()
  @ApiBearerAuth()
  @AuthGuard('project')
  @TokenReadAccess()
  async list(@Request() request: PlatformRequest) {
    const projectId = request.projectId as string
    return this.importRunsService.listRuns(projectId)
  }

  @Get(':runId')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @AuthGuard('project')
  @TokenReadAccess()
  async detail(@Request() request: PlatformRequest, @Param('runId') runId: string) {
    const projectId = request.projectId as string
    return this.importRunsService.getRunDetail(runId, projectId)
  }

  @Post(':runId/start')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async start(@Request() request: PlatformRequest, @Param('runId') runId: string) {
    await this.importRunsService.startRun(runId, request.projectId as string)
    return { status: 'queued' }
  }

  @Post(':runId/cancel')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async cancel(@Request() request: PlatformRequest, @Param('runId') runId: string) {
    await this.importRunsService.cancelRun(runId, request.projectId as string)
    return { status: 'canceling' }
  }

  @Post(':runId/retry')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @HttpCode(HttpStatus.ACCEPTED)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async retry(@Request() request: PlatformRequest, @Param('runId') runId: string) {
    await this.importRunsService.retryRun(runId, request.projectId as string)
    return { status: 'queued' }
  }

  @Delete(':runId')
  @ApiBearerAuth()
  @ApiParam({ name: 'runId' })
  @HttpCode(HttpStatus.NO_CONTENT)
  @UseGuards(EntityWriteGuard)
  @AuthGuard('project')
  async deleteDraft(@Request() request: PlatformRequest, @Param('runId') runId: string): Promise<void> {
    await this.importRunsService.deleteDraftRun(runId, request.projectId as string)
  }
}
