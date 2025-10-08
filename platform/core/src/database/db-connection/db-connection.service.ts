import { Injectable, Logger } from '@nestjs/common'
import { Neogma } from 'neogma'

import { isDevMode } from '@/common/utils/isDevMode'
import { IProjectProperties } from '@/dashboard/project/model/project.interface'
import { ProjectService } from '@/dashboard/project/project.service'
import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'
import { LOCAL_PROJECT_CONNECTION_LITERAL } from '@/database/db-connection/db-connection.constants'

import { INeogmaConfig } from '../neogma/neogma-config.interface'
import { NeogmaService } from '../neogma/neogma.service'
import { NeogmaDynamicService } from '../neogma-dynamic/neogma-dynamic.service'

export interface ConnectionResult {
  connection: Neogma
  projectId: string | typeof LOCAL_PROJECT_CONNECTION_LITERAL
}

@Injectable()
export class DbConnectionService {
  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly neogmaDynamicService: NeogmaDynamicService,
    private readonly projectService: ProjectService
  ) {}

  async getConnection(projectId: string, project?: IProjectProperties): Promise<ConnectionResult> {
    if (projectId === LOCAL_PROJECT_CONNECTION_LITERAL) {
      isDevMode(() => Logger.debug(`Using default connection for project`))

      return { connection: this.neogmaService.getInstance(), projectId: LOCAL_PROJECT_CONNECTION_LITERAL }
    }

    if (project?.customDb) {
      try {
        const customDbPayload = this.projectService.decryptSensitiveData<TProjectCustomDbPayload>(
          project.customDb
        )

        const config: INeogmaConfig = {
          url: customDbPayload.url,
          username: customDbPayload.username,
          password: customDbPayload.password
        }

        isDevMode(() => Logger.debug(`Using dynamic connection for project ${projectId}`))

        const externalConnection = await this.neogmaDynamicService.getConnection(projectId, config)

        return { connection: externalConnection, projectId }
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
    return { connection: this.neogmaService.getInstance(), projectId: LOCAL_PROJECT_CONNECTION_LITERAL }
  }
}
