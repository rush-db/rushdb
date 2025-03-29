import { Injectable, OnApplicationShutdown } from '@nestjs/common'
import { Driver, Session } from 'neo4j-driver'
import { Neogma, QueryBuilder, QueryRunner } from 'neogma'
import { dbContextStorage } from '../db-context'
import { NeogmaService } from '../neogma/neogma.service'

@Injectable()
export class CompositeNeogmaService implements OnApplicationShutdown {
  constructor(private readonly defaultNeogmaService: NeogmaService) {}

  private getCurrentInstance(): Neogma {
    const context = dbContextStorage.getStore()
    return context && context.connection ? context.connection : this.defaultNeogmaService.getInstance()
  }

  getDriver(): Driver {
    return this.getCurrentInstance().driver as unknown as Driver
  }

  getInstance(): Neogma {
    return this.getCurrentInstance()
  }

  getConfig() {
    return this.defaultNeogmaService.getConfig()
  }

  async activeSessions() {
    return this.defaultNeogmaService.activeSessions()
  }

  createSession(): Session {
    return this.getCurrentInstance().driver.session()
  }

  async closeSession(session: Session) {
    await session?.close()
  }

  getReadSession(): Session {
    return this.getCurrentInstance().driver.session({ defaultAccessMode: 'READ' })
  }

  getWriteSession(): Session {
    return this.getCurrentInstance().driver.session({ defaultAccessMode: 'WRITE' })
  }

  createRunner(): QueryRunner {
    const options = this.getConfig().mode === 'production' ? {} : {}
    return new QueryRunner({
      driver: this.getDriver()
    })
  }

  createBuilder(): QueryBuilder {
    return new QueryBuilder()
  }

  async onApplicationShutdown() {
    return this.getDriver().close()
  }
}
