import { CallHandler, ExecutionContext, Injectable } from '@nestjs/common'
import axios from 'axios'
import { Transaction } from 'neo4j-driver'
import { of } from 'rxjs'
import { parser } from 'stream-json'
import { streamValues } from 'stream-json/streamers/StreamValues'

import { ESideEffectType, RunSideEffectMixin } from '@/common/interceptors/run-side-effect.interceptor'
import { isArray } from '@/common/utils/isArray'
import { ImportService } from '@/core/entity/import-export/import.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { createReadStream } from 'fs'

@Injectable()
export class BackupService {
  constructor(
    private readonly importService: ImportService,
    private readonly neogmaService: NeogmaService,
    private readonly projectService: ProjectService
  ) {}

  async restore({
    filePath,
    transaction,
    projectId,
    batchSize = 100
  }: {
    filePath: string
    transaction: Transaction
    projectId: string
    batchSize?: number
  }): Promise<void> {
    let pipeline

    if (filePath.startsWith('http')) {
      const response = await axios.get(filePath, { responseType: 'stream' })
      pipeline = response.data.pipe(parser()).pipe(streamValues())
    } else {
      pipeline = createReadStream(filePath).pipe(parser()).pipe(streamValues())
    }

    let records: any[] = []
    let relationships: any[] = []

    for await (const data of pipeline) {
      if (
        data.value &&
        'records' in data.value &&
        'relationships' in data.value &&
        isArray(data.value.relationships) &&
        isArray(data.value.records)
      ) {
        const { records: fileRecords, relationships: fileRelationships } = data.value
        // Batch process records
        for (const record of fileRecords) {
          records.push(record)
          if (records.length >= batchSize) {
            await this.processRecords({ batch: records, transaction, projectId })
            records = []
          }
        }
        if (records.length > 0) {
          await this.processRecords({ batch: records, transaction, projectId })
        }

        // Batch process relationships
        for (const relationship of fileRelationships) {
          relationships.push(relationship)
          if (relationships.length >= batchSize) {
            await this.processRelationships({ batch: relationships, projectId, transaction })
            relationships = []
          }
        }
        if (relationships.length > 0) {
          await this.processRelationships({ batch: relationships, projectId, transaction })
        }

        break // Exit after processing the top-level JSON structure
      }
    }
    console.log('File processing completed.')
  }

  private async processRelationships({
    batch,
    projectId,
    transaction
  }: {
    batch: any[]
    projectId: string
    transaction: Transaction
  }): Promise<void> {
    console.log(`Processing Relationships batch of size: ${batch.length}`)
    await this.importService.processRelationshipsChunk({ relationsChunk: batch, projectId, transaction })
  }

  private async processRecords({
    batch,
    projectId,
    transaction
  }: {
    batch: any[]
    projectId: string
    transaction: Transaction
  }): Promise<void> {
    console.log(`Processing Records batch of size: ${batch.length}`)
    await this.importService.importRecords({ payload: batch, label: '' }, projectId, transaction)
  }

  async runSideEffects(projectId: string): Promise<void> {
    const mockExecutionContext: Partial<ExecutionContext> = {
      switchToHttp: () => ({
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-expect-error
        getRequest: () => ({ projectId })
      })
    }

    const SideEffectInterceptor = RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE])
    const sideEffectInstance = new SideEffectInterceptor(this.neogmaService, this.projectService)

    // Simulate CallHandler with an empty observable
    const mockCallHandler: CallHandler = {
      handle: () => of(null)
    }

    // Invoke the interceptor
    await sideEffectInstance.intercept(mockExecutionContext as ExecutionContext, mockCallHandler)
  }
}
