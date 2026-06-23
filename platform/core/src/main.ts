import { RequestMethod } from '@nestjs/common'
import { CorsOptions } from '@nestjs/common/interfaces/external/cors-options.interface'
import { NestFactory } from '@nestjs/core'
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify'
import { DocumentBuilder, SwaggerModule } from '@nestjs/swagger'
import fastifyRawBody from 'fastify-raw-body'

import { GlobalExceptionFilter } from '@/common/global-exception.filter'
import { TransactionService } from '@/core/transactions/transaction.service'
import { AuthMiddleware } from '@/dashboard/auth/middlewares/auth.middleware'

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
    'Content-Disposition'
  ],
  exposedHeaders: ['Authorization', 'Content-Disposition'],
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

  // NOTE: @nestjs/platform-fastify v11 registers an
  // application/x-www-form-urlencoded body parser by default, which covers the
  // SAML HTTP-POST binding (SAMLResponse / RelayState fields delivered to the
  // ACS endpoint). No explicit @fastify/formbody registration is needed.

  const app = await NestFactory.create<NestFastifyApplication>(AppModule, fastifyAdapter, {})

  app.setGlobalPrefix('api/v1', {
    exclude: [
      { path: '/', method: RequestMethod.GET },
      { path: 'health', method: RequestMethod.GET },
      { path: '.well-known/*path', method: RequestMethod.GET },
      { path: 'oauth/*path', method: RequestMethod.GET },
      { path: 'oauth/*path', method: RequestMethod.POST },
      { path: 'oauth/*path', method: RequestMethod.DELETE },
      { path: 'oauth/*path', method: RequestMethod.PATCH }
    ]
  })
  app.enableCors(CORS_OPTIONS)

  // The OAuth routes are excluded from the global prefix so MCP clients can
  // reach them at the root. In NestJS 11 the `forRoutes('*')` middleware
  // registered by the feature modules no longer extends to prefix-excluded
  // routes, so dashboard-authenticated OAuth endpoints (e.g. /oauth/consents)
  // never had their user populated. Apply AuthMiddleware explicitly to /oauth
  // here; the global RequestCleanupInterceptor still commits/closes any Neo4j
  // session it opens.
  const authMiddleware = app.get(AuthMiddleware, { strict: false })
  app.use('/oauth', (req: unknown, res: unknown, next: () => void) =>
    authMiddleware.use(req as never, res as never, next)
  )

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

  await app.listen(process.env['RUSHDB_PORT'] || process.env['PORT'] || 3000, '0.0.0.0')
}

void bootstrap()
