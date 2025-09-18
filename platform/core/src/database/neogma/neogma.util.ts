import { Logger } from '@nestjs/common'
import { LogLevel } from 'neo4j-driver-core/types/types'
import { Neogma } from 'neogma'

import { INeogmaConfig } from './neogma-config.interface'

let CONNECTIONS = 0

export const createInstance = async (config: INeogmaConfig): Promise<Neogma> => {
  CONNECTIONS++
  return new Neogma(
    {
      url: config.url,
      username: config.username,
      password: config.password
    },
    {
      maxConnectionPoolSize: 0,
      logging: {
        level: 'debug',
        logger:
          // LOG External Connections only
          config.url === process.env.NEO4J_URL ?
            () => {}
          : (level, message) => Logger.debug(new Date().toISOString() + ': ' + level + ' ' + message)
      }
    }
  )
}
