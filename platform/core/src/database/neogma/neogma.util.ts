import { Neogma } from 'neogma'

import { INeogmaConfig } from './neogma-config.interface'

export const createInstance = async (config: INeogmaConfig): Promise<Neogma> => {
  return new Neogma(
    {
      url: config.url,
      username: config.username,
      password: config.password
    },
    {
      maxConnectionPoolSize: 0
      // logging: {
      //     level: 'debug',
      //     logger: (level, message) =>
      //         console.log(new Date().toISOString() + ': ' + level + ' ' + message),
      // },
    }
  )
}