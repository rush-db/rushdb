import { RequestMethod } from '@nestjs/common'
import { CorsOptions } from '@nestjs/common/interfaces/external/cors-options.interface'
import { NestFactory } from '@nestjs/core'
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify'
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger'
import fastifyRawBody from 'fastify-raw-body'

import { GlobalExceptionFilter } from '@/common/global-exception.filter'
import { TransactionService } from '@/core/transactions/transaction.service'

import { AppModule } from './app.module'

const CORS_OPTIONS: CorsOptions = {
  origin: true,
  allowedHeaders: [
    'Access-Control-Allow-Origin',
    'Origin',
    'X-Requested-With',
    'Accept',
    'Content-Type',
    'Authorization',
    'X-Workspace-Id',
    'X-Project-Id',
    'X-Transaction-Id',
    'Content-Disposition',
    'Token'
  ],
  exposedHeaders: ['Authorization', 'Content-Disposition', 'Token'],
  credentials: true,
  methods: ['GET', 'PUT', 'OPTIONS', 'POST', 'PATCH', 'DELETE']
}

async function bootstrap() {
  const fastifyAdapter = new FastifyAdapter({
    logger: true,
    bodyLimit: 1024 * 1024 * 1024 * 2 // 2GB
  })

  await fastifyAdapter.register(fastifyRawBody, {
    runFirst: true,
    routes: ['/api/v1/billing/payment/webhook']
  })

  const app = await NestFactory.create<NestFastifyApplication>(AppModule, fastifyAdapter, {})

  app.setGlobalPrefix('api/v1', {
    exclude: [{ path: '/', method: RequestMethod.GET }]
  })
  app.enableCors(CORS_OPTIONS)

  const config = new DocumentBuilder()
    .setTitle('RushDB API')
    .setDescription('RushDB API specs')
    .setVersion('1.0')
    .addBearerAuth()
    .build()

  const document = SwaggerModule.createDocument(app, config)
  SwaggerModule.setup('api', app, document)

  const transactionService = app.get<TransactionService>(TransactionService)

  app.useGlobalFilters(new GlobalExceptionFilter(transactionService))

  await app.listen(process.env['RUSHDB_PORT'] || 3000, '0.0.0.0')
}

void bootstrap()
