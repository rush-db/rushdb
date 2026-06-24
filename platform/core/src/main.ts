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

  // Initialise explicitly so @nestjs/platform-fastify registers its own
  // application/json and application/x-www-form-urlencoded parsers first
  // (it marks them registered, so we must not pre-register or it throws
  // FST_ERR_CTP_ALREADY_PRESENT). init() does not boot Fastify (no ready()),
  // so content-type parsers can still be swapped before listen().
  await app.init()

  // Fastify 5's built-in application/json parser rejects an empty body with
  // "Body cannot be empty when content-type is set to 'application/json'",
  // returning 400 before the route handler runs. Clients (the published SDK
  // and the dashboard) send bodyless requests — e.g. DELETE /projects/:id —
  // that still carry a `Content-Type: application/json` header, which broke
  // after the Fastify 4 → 5 upgrade. Replace ONLY the json parser so an empty
  // body is parsed as `undefined` (no body) instead of erroring; non-empty
  // bodies are still strictly JSON-parsed, and endpoints that require a body
  // continue to be enforced by their validation pipes. The urlencoded parser
  // (used by the SAML HTTP-POST binding) is left untouched.
  const fastifyInstance = fastifyAdapter.getInstance()
  fastifyInstance.removeContentTypeParser('application/json')
  fastifyInstance.addContentTypeParser(
    'application/json',
    { parseAs: 'string' },
    (_req, body: string | Buffer, done) => {
      const raw = typeof body === 'string' ? body : body.toString()
      if (raw.trim() === '') {
        done(null, undefined)
        return
      }
      try {
        done(null, JSON.parse(raw))
      } catch (err) {
        ;(err as Error & { statusCode?: number }).statusCode = 400
        done(err as Error, undefined)
      }
    }
  )

  await app.listen(process.env['RUSHDB_PORT'] || process.env['PORT'] || 3000, '0.0.0.0')
}

void bootstrap()
