import envelopeV1 from '../schema/synx-envelope-v1.schema.json'
import acknowledgementV1 from '../schema/synx-ack-v1.schema.json'
import capabilitiesV1 from '../schema/synx-capabilities-v1.schema.json'
import connectorV1 from '../schema/synx-connector-v1.schema.json'

/** Schema set keyed by canonical filename; see `computeSchemaHash`. */
export const SCHEMAS_V1: Record<string, unknown> = {
  'synx-ack-v1.schema.json': acknowledgementV1,
  'synx-capabilities-v1.schema.json': capabilitiesV1,
  'synx-connector-v1.schema.json': connectorV1,
  'synx-envelope-v1.schema.json': envelopeV1
}

export { envelopeV1, acknowledgementV1, capabilitiesV1, connectorV1 }
