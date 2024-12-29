import { Inject, Injectable, OnApplicationShutdown } from '@nestjs/common'
import { Driver, Session } from 'neo4j-driver'
import { Neogma, QueryBuilder, QueryRunner } from 'neogma'

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
    const transaction = session.beginTransaction()

    const res = await transaction.run('CALL dbms.listConnections()')

    await transaction.close()
    await session.close()

    return res
  }

  createSession(): Session {
    // this.activeSessions().then((activeSessions) =>
    //     console.log('Active Sessions (before): ', activeSessions.records.length)
    // );
    return this.getDriver().session()
  }

  async closeSession(session: Session, context?: any) {
    await session?.close()
    // const activeSessions = await this.activeSessions();
    // console.log('Active Sessions (close): ', activeSessions.records.length, context);
  }

  getReadSession() {
    return this.instance.driver.session({
      defaultAccessMode: 'READ'
    })
  }

  getWriteSession() {
    return this.instance.driver.session({
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
