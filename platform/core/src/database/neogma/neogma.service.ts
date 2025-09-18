import { Inject, Injectable, Logger, OnApplicationShutdown } from '@nestjs/common'
import { Driver, Session } from 'neo4j-driver'
import { Neogma, QueryBuilder, QueryRunner } from 'neogma'

import { isDevMode } from '@/common/utils/isDevMode'

import { INeogmaConfig } from './neogma-config.interface'
import { NEOGMA_CONFIG, NEOGMA_INSTANCE } from './neogma.constants'

@Injectable()
export class NeogmaService implements OnApplicationShutdown {
  constructor(
    @Inject(NEOGMA_CONFIG) private readonly config: INeogmaConfig,
    @Inject(NEOGMA_INSTANCE) private readonly instance: Neogma
  ) {}

  getDriver(): Driver {
    return this.instance.driver as unknown as Driver
  }

  getInstance() {
    return this.instance
  }

  getConfig(): INeogmaConfig {
    return this.config
  }

  async activeSessions() {
    const session = this.getDriver().session()
    const transaction = session.beginTransaction({ timeout: 10_000 })

    const res = await transaction.run('CALL dbms.listConnections()')

    await transaction.close()
    await session.close()

    return res
  }

  async activeTransactions() {
    const session = this.getDriver().session()
    const transaction = session.beginTransaction({ timeout: 10_000 })

    const res = await transaction.run('SHOW TRANSACTIONS')

    await transaction.close()
    await session.close()

    return res
  }

  async stats(context?: any) {
    try {
      const [activeSessions, activeTransactions] = await Promise.all([
        this.activeSessions(),
        this.activeTransactions()
      ])
      Logger.debug(
        `[NEO4J STATS] (context: ${context}): ${JSON.stringify({ activeSessions: activeSessions.records.length, activeTransactions: activeTransactions.records.length })}`
      )
    } catch (error) {
      Logger.error(`[NEO4J STATS] (context: ${context}): unable to get stats`)
    }
  }

  createSession(context?: any): Session {
    const session = this.getDriver().session()
    // isDevMode(() => {
    //   this.stats('create session ' + (context ? context : ''))
    // })

    return session
  }

  async closeSession(session: Session, context?: any) {
    await session?.close()

    // isDevMode(() => {
    //   this.stats('close session ' + (context ? context : ''))
    // })
  }

  getReadSession() {
    return this.getDriver().session({
      defaultAccessMode: 'READ'
    })
  }

  getWriteSession() {
    return this.getDriver().session({
      defaultAccessMode: 'WRITE'
    })
  }

  createRunner(): QueryRunner {
    const options = this.config.mode === 'production' ? {} : {}
    return new QueryRunner({
      driver: this.getDriver(),
      ...options
    })
  }

  createBuilder(): QueryBuilder {
    return new QueryBuilder()
  }

  onApplicationShutdown() {
    return this.getDriver().close()
  }
}
