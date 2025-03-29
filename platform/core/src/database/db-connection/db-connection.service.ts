import { Injectable, Logger } from '@nestjs/common'
import { Neogma } from 'neogma'
import { NeogmaService } from '../neogma/neogma.service'
import { NeogmaDynamicService } from '../neogma-dynamic/neogma-dynamic.service'
import { INeogmaConfig } from '../neogma/neogma-config.interface'
import { ProjectService } from '@/dashboard/project/project.service'
import { Transaction } from 'neo4j-driver'
import { transactionStorage } from '@/core/transactions/transaction-context'
import { isDevMode } from '@/common/utils/isDevMode'

@Injectable()
export class DbConnectionService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly neogmaDynamicService: NeogmaDynamicService,
    private readonly projectService: ProjectService
  ) {}

  async getConnection(projectId: string): Promise<Neogma> {
    const context = transactionStorage.getStore()
    const tx: Transaction | undefined = context?.transaction

    if (projectId === 'default') {
      isDevMode(() => Logger.debug(`Using default connection for project`))

      return this.neogmaService.getInstance()
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

        return await this.neogmaDynamicService.getConnection(projectId, config)
      } catch (error) {
        isDevMode(() =>
          Logger.error(
            `Failed to use dynamic connection for project ${projectId}: ${error}`,
            undefined,
            'DbConnectionService'
          )
        )

        return this.neogmaService.getInstance()
      }
    }

    isDevMode(() => Logger.debug(`Using default connection for project ${projectId}`))
    return this.neogmaService.getInstance()
  }
}
