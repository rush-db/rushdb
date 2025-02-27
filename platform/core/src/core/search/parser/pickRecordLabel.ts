import { RUSHDB_KEY_LABEL, RUSHDB_LABEL_RECORD, DEFAULT_RECORD_ALIAS } from '@/core/common/constants'

export const label = (alias = DEFAULT_RECORD_ALIAS, key: string = RUSHDB_KEY_LABEL) =>
  `${key}: [label IN labels(${alias}) WHERE label <> "${RUSHDB_LABEL_RECORD}"][0]`
