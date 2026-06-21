import { INestApplication } from '@nestjs/common'
import { FastifyAdapter } from '@nestjs/platform-fastify'
import { Test, TestingModule } from '@nestjs/testing'
import * as request from 'supertest'

import { AppModule } from '@/app.module'

describe('AppController (e2e)', () => {
  let app: INestApplication

  beforeEach(async () => {
    const moduleFixture: TestingModule = await Test.createTestingModule({
      imports: [AppModule]
    }).compile()

    app = moduleFixture.createNestApplication(new FastifyAdapter())
    await app.init()
    await app.getHttpAdapter().getInstance().ready()
  })

  it('/ (GET)', () => {
    return request(app.getHttpServer()).get('/').expect(200).expect('RushDB is running')
  })

  it('/health (GET)', () => {
    return request(app.getHttpServer()).get('/health').expect(200).expect({ status: 'ok' })
  })
})
