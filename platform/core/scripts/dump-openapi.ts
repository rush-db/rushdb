import { NestFactory } from '@nestjs/core'
import { FastifyAdapter, NestFastifyApplication } from '@nestjs/platform-fastify'
import { SwaggerModule } from '@nestjs/swagger'

import { AppModule } from '@/app.module'
import { buildSwaggerConfig, GLOBAL_PREFIX, GLOBAL_PREFIX_OPTIONS } from '@/common/swagger'

import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

/**
 * Dumps the OpenAPI spec served at /api to platform/core/openapi.json without
 * starting the HTTP server. The app is created but never init()'d or
 * listen()'d, so lifecycle hooks (Neo4j constraint seeding, SQL migrations)
 * never run and no live database connection is required — Swagger only
 * inspects the module graph.
 */
async function dumpOpenApi() {
  const app = await NestFactory.create<NestFastifyApplication>(
    AppModule,
    new FastifyAdapter({ logger: false }),
    { logger: false, abortOnError: false }
  )

  app.setGlobalPrefix(GLOBAL_PREFIX, GLOBAL_PREFIX_OPTIONS)

  const document = SwaggerModule.createDocument(app, buildSwaggerConfig())

  const outputPath = resolve(__dirname, '..', 'openapi.json')
  writeFileSync(outputPath, JSON.stringify(document, null, 2) + '\n')

  process.stdout.write(`OpenAPI spec written to ${outputPath}\n`)

  // The app was never initialised, so there is nothing to close — exit
  // directly instead of app.close() to avoid triggering shutdown hooks on
  // providers that never started.
  process.exit(0)
}

void dumpOpenApi().catch((error) => {
  console.error('Failed to dump OpenAPI spec:', error)
  process.exit(1)
})
