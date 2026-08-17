import { Body, Controller, Get, Headers, Post } from '@nestjs/common'
import { ApiExcludeController } from '@nestjs/swagger'
import {
  SynxAcknowledgementV1,
  SynxConnectorCatalogV1,
  SynxConnectorV1,
  SynxEnvelopeV1
} from '@rushdb/synx-contract'

import { SynxConnectorRegistry } from './synx.connectors'
import { SynxService } from './synx.service'

/**
 * Internal synx destination. Registered under the `api/v1` global prefix →
 * `POST /api/v1/_internal/synx/batches`. Authenticated by the
 * `x-synx-control-token` header, never by a project bearer token, so it is
 * excluded from the public API surface and passes through AuthMiddleware
 * unauthenticated (see connector `_internal` routes for the same pattern).
 */
@Controller('_internal/synx')
@ApiExcludeController()
export class SynxController {
  constructor(
    private readonly synxService: SynxService,
    private readonly connectorRegistry: SynxConnectorRegistry
  ) {}

  @Post('batches')
  applyBatch(
    @Body() envelope: SynxEnvelopeV1,
    @Headers('x-synx-control-token') controlToken?: string
  ): Promise<SynxAcknowledgementV1> {
    return this.synxService.applyBatch(envelope, controlToken)
  }

  /**
   * Worker registers its connector catalog (upsert by id). This is how Core
   * learns the provider union — no hardcoded references, no restart.
   */
  @Post('connectors/register')
  register(
    @Body() body: { connectors?: SynxConnectorV1[] },
    @Headers('x-synx-control-token') controlToken?: string
  ): Promise<SynxConnectorCatalogV1> {
    this.synxService.assertWorkerToken(controlToken)
    return this.connectorRegistry.register(body?.connectors ?? [])
  }

  @Get('connectors')
  connectors(): Promise<SynxConnectorCatalogV1> {
    return this.connectorRegistry.list()
  }
}
