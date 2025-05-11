import { Injectable, Logger } from '@nestjs/common'
import { Neogma } from 'neogma'
import { NeogmaService } from '../neogma/neogma.service'
import { NeogmaDynamicService } from '../neogma-dynamic/neogma-dynamic.service'
import { INeogmaConfig } from '../neogma/neogma-config.interface'
import { ProjectService } from '@/dashboard/project/project.service'
import { Transaction } from 'neo4j-driver'
import { transactionStorage } from '@/core/transactions/transaction-context'
import { isDevMode } from '@/common/utils/isDevMode'

export interface ConnectionResult {
  connection: Neogma
  projectId: string | 'default'
}

@Injectable()
export class DbConnectionService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly neogmaDynamicService: NeogmaDynamicService,
    private readonly projectService: ProjectService
  ) {}

  async getConnection(projectId: string): Promise<ConnectionResult> {
    const context = transactionStorage.getStore()
    const tx: Transaction | undefined = context?.transaction

    if (projectId === 'default') {
      isDevMode(() => Logger.debug(`Using default connection for project`))

      return { connection: this.neogmaService.getInstance(), projectId: 'default' }
    }

    const projectNode = await this.projectService.getProject(projectId, tx)
    const project = projectNode.toJson()

    if (project?.customDb) {
      try {
        const customDbPayload = this.projectService.decryptCustomDb(project.customDb)

        const config: INeogmaConfig = {
          url: customDbPayload.url,
          username: customDbPayload.username,
          password: customDbPayload.password
        }

        isDevMode(() => Logger.debug(`Using dynamic connection for project ${projectId}`))

        const dynamicConnection = await this.neogmaDynamicService.getConnection(projectId, config)

        return { connection: dynamicConnection, projectId }
      } catch (error) {
        isDevMode(() =>
          Logger.error(
            `Failed to use dynamic connection for project ${projectId}: ${error}`,
            undefined,
            'DbConnectionService'
          )
        )

        throw new Error(`Dynamic connection failed for project ${projectId}: ${error}`)
      }
    }

    isDevMode(() => Logger.debug(`Using default connection for project ${projectId}`))
    return { connection: this.neogmaService.getInstance(), projectId: 'default' }
  }
}
