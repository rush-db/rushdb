import { BadRequestException, Injectable, Logger, ServiceUnavailableException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import axios from 'axios'

import { AiService } from '@/core/ai/ai.service'
import { SchemaItem } from '@/core/ai/ai.types'
import { SearchDto } from '@/core/search/dto/search.dto'
import { resolveMaxTraversalHops } from '@/core/search/search.constants'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

import { existsSync, readFileSync } from 'fs'
import { join } from 'path'

type GeneratedSearchQuery = {
  searchQuery?: SearchDto
  warnings?: string[]
}

type ValidationResult = {
  query: SearchDto
  warnings: string[]
  errors: string[]
}

const TOP_LEVEL_KEYS = new Set([
  'labels',
  'where',
  'select',
  'aggregate',
  'groupBy',
  'orderBy',
  'skip',
  'limit'
])
const LOGICAL_KEYS = new Set(['$and', '$or', '$not', '$nor', '$xor'])
const COMPARISON_KEYS = new Set([
  '$eq',
  '$ne',
  '$gt',
  '$gte',
  '$lt',
  '$lte',
  '$contains',
  '$startsWith',
  '$endsWith',
  '$in',
  '$nin',
  '$exists',
  '$type',
  '$year',
  '$month',
  '$day',
  '$hour',
  '$minute',
  '$second',
  '$millisecond',
  '$microsecond',
  '$nanosecond'
])
const RELATION_META_KEYS = new Set(['$alias', '$relation'])
// A where-predicate value that looks like a field/alias reference, e.g. "$record.id" or "$alias.name".
// Such references are only valid in select/groupBy/aggregate. Used as a where VALUE they are matched as a
// literal string (see parseComparison), which silently returns no rows — so we flag them during validation.
const FIELD_REF_VALUE_REGEX = /^\$[A-Za-z_]\w*(\.\w+)*$/
const SELECT_OPERATOR_KEYS = new Set([
  '$ref',
  '$sum',
  '$avg',
  '$count',
  '$min',
  '$max',
  '$divide',
  '$multiply',
  '$add',
  '$subtract',
  '$collect',
  '$timeBucket'
])

const QUERY_BUILDER_PROMPT_FILE = 'search-query-builder.prompt.md'
const SEARCH_QUERY_SPEC_PROMPT_FILE = 'search-query-spec.prompt.md'

@Injectable()
export class SearchQueryGeneratorService {
  private readonly logger = new Logger(SearchQueryGeneratorService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly aiService: AiService
  ) {}

  async generate({
    prompt,
    currentQuery,
    projectId,
    workspaceId
  }: {
    prompt: string
    currentQuery?: SearchDto
    projectId: string
    workspaceId?: string
  }): Promise<GeneratedSearchQuery> {
    if (!this.enabled()) {
      throw new ServiceUnavailableException(
        'AI search is disabled. Set RUSHDB_LLM_API_KEY and RUSHDB_LLM_MODEL.'
      )
    }

    const trimmed = prompt?.trim()
    if (!trimmed) {
      throw new BadRequestException('Prompt is required.')
    }

    // allowStale: query generation is a hot interactive path — a slightly stale schema
    // is fine for prompting the LLM, and a synchronous full-graph recompute here would
    // stack on top of the LLM round-trip latency.
    const schema = await this.aiService.getSchema({ projectId, workspaceId, allowStale: true })
    const schemaMarkdown = this.aiService.buildMdSchema(schema)
    const first = await this.callLlm({ prompt: trimmed, currentQuery, schemaMarkdown })
    this.logger.log(`[AI SearchQuery] generated raw query: ${JSON.stringify(first.searchQuery)}`)
    let validated = this.validateAndNormalize(first.searchQuery, schema)

    if (validated.errors.length) {
      this.logger.warn(`[AI SearchQuery] validation errors: ${validated.errors.join('; ')}`)
      const repaired = await this.callLlm({
        prompt: trimmed,
        currentQuery,
        schemaMarkdown,
        validationErrors: validated.errors,
        previousQuery: first.searchQuery
      })
      this.logger.log(`[AI SearchQuery] generated repair query: ${JSON.stringify(repaired.searchQuery)}`)
      validated = this.validateAndNormalize(repaired.searchQuery, schema)
    }

    if (validated.errors.length) {
      throw new BadRequestException(`Generated query is not valid: ${validated.errors.join('; ')}`)
    }

    this.logger.log(`[AI SearchQuery] final query: ${JSON.stringify(validated.query)}`)

    return {
      searchQuery: validated.query,
      warnings: [...(first.warnings ?? []), ...validated.warnings]
    }
  }

  private enabled(): boolean {
    return (
      Boolean(this.configService.get<string>('RUSHDB_LLM_API_KEY')) &&
      Boolean(this.configService.get<string>('RUSHDB_LLM_MODEL'))
    )
  }

  private readPromptFile(fileName: string): string {
    const candidates = [
      join(__dirname, fileName),
      join(process.cwd(), 'src/core/ai', fileName),
      join(process.cwd(), 'platform/core/src/core/ai', fileName)
    ]
    const promptPath = candidates.find((candidate) => existsSync(candidate))

    if (!promptPath) {
      throw new Error(`Missing ${fileName}`)
    }

    return readFileSync(promptPath, 'utf8')
  }

  private async callLlm({
    prompt,
    currentQuery,
    schemaMarkdown,
    validationErrors,
    previousQuery
  }: {
    prompt: string
    currentQuery?: SearchDto
    schemaMarkdown: string
    validationErrors?: string[]
    previousQuery?: SearchDto
  }): Promise<GeneratedSearchQuery> {
    const apiKey = this.configService.get<string>('RUSHDB_LLM_API_KEY')
    const model = this.configService.get<string>('RUSHDB_LLM_MODEL')
    const baseUrl = this.configService.get<string>('RUSHDB_LLM_BASE_URL') ?? 'https://api.openai.com/v1'
    const queryBuilderPrompt = this.readPromptFile(QUERY_BUILDER_PROMPT_FILE)
    const searchQuerySpecPrompt = this.readPromptFile(SEARCH_QUERY_SPEC_PROMPT_FILE)

    const response = await axios.post(
      `${baseUrl.replace(/\/$/, '')}/chat/completions`,
      {
        model,
        temperature: 0,
        response_format: { type: 'json_object' },
        messages: [
          {
            role: 'system',
            content: `${queryBuilderPrompt}\n\n---\n\n${searchQuerySpecPrompt}`
          },
          {
            role: 'user',
            content: JSON.stringify({
              task: 'Build one RushDB SearchQuery for the prompt using only the provided schema. Return only JSON in the exact shape {"searchQuery":{...},"warnings":[...]}.',
              prompt,
              currentQuery,
              schemaMarkdown,
              previousQuery,
              validationErrors
            })
          }
        ]
      },
      {
        headers: {
          Authorization: `Bearer ${apiKey}`,
          'Content-Type': 'application/json'
        },
        timeout: DEFAULT_TRANSACTION_TIMEOUT_MS
      }
    )

    const choice = response.data?.choices?.[0]
    const content = choice?.message?.content
    if (!content) {
      throw new BadRequestException('AI did not return a query.')
    }

    try {
      const parsed = this.parseAiJson(content)
      return {
        searchQuery: parsed?.searchQuery,
        warnings: Array.isArray(parsed?.warnings) ? parsed.warnings.filter((w) => typeof w === 'string') : []
      }
    } catch (error) {
      const finishReason = choice?.finish_reason ? ` finish_reason=${choice.finish_reason}` : ''
      this.logger.warn(
        `Failed to parse AI SearchQuery response.${finishReason} error=${String(error)} preview=${this.previewAiContent(content)}`
      )
      throw new BadRequestException('AI returned invalid JSON.')
    }
  }

  private parseAiJson(content: string): GeneratedSearchQuery {
    const trimmed = content.trim()
    const candidates = [
      trimmed,
      trimmed
        .replace(/^```(?:json)?\s*/i, '')
        .replace(/\s*```$/i, '')
        .trim(),
      this.extractFirstJsonObject(trimmed)
    ].filter((candidate): candidate is string => Boolean(candidate))

    let lastError: unknown
    for (const candidate of candidates) {
      try {
        return JSON.parse(candidate)
      } catch (error) {
        lastError = error
      }
    }

    throw lastError ?? new Error('No JSON object found')
  }

  private extractFirstJsonObject(content: string): string | undefined {
    const start = content.indexOf('{')
    if (start === -1) {
      return undefined
    }

    let depth = 0
    let inString = false
    let escaped = false

    for (let index = start; index < content.length; index += 1) {
      const char = content[index]

      if (escaped) {
        escaped = false
        continue
      }

      if (char === '\\') {
        escaped = true
        continue
      }

      if (char === '"') {
        inString = !inString
        continue
      }

      if (inString) {
        continue
      }

      if (char === '{') {
        depth += 1
      } else if (char === '}') {
        depth -= 1
        if (depth === 0) {
          return content.slice(start, index + 1)
        }
      }
    }

    return undefined
  }

  private previewAiContent(content: string): string {
    const normalized = content.replace(/\s+/g, ' ').trim()
    return JSON.stringify(normalized.length > 300 ? `${normalized.slice(0, 300)}...` : normalized)
  }

  private validateAndNormalize(searchQuery: unknown, schema: SchemaItem[]): ValidationResult {
    const warnings: string[] = []
    const errors: string[] = []

    if (!searchQuery || typeof searchQuery !== 'object' || Array.isArray(searchQuery)) {
      return { query: {}, warnings, errors: ['searchQuery must be an object'] }
    }

    const query = JSON.parse(JSON.stringify(searchQuery)) as SearchDto
    this.normalizeOrderBy(query, warnings)
    this.normalizeTraversalHops(query.where, warnings)
    for (const key of Object.keys(query)) {
      if (!TOP_LEVEL_KEYS.has(key)) {
        errors.push(`Unsupported top-level key "${key}"`)
      }
    }

    const schemaByLabel = new Map(schema.map((item) => [item.label, item]))
    const propertiesByLabel = new Map(
      schema.map((item) => [item.label, new Set(item.properties.map((property) => property.name))])
    )
    const allProperties = new Set(schema.flatMap((item) => item.properties.map((property) => property.name)))
    const labels =
      Array.isArray(query.labels) ? query.labels.filter((label) => typeof label === 'string') : []

    for (const label of labels) {
      if (!schemaByLabel.has(label)) {
        errors.push(`Unknown label "${label}"`)
      }
    }

    if (this.selectRequiresFullScan(query)) {
      if ('limit' in query) {
        delete query.limit
        warnings.push('Removed limit because select queries must scan the full result set.')
      }
      if ('skip' in query) {
        delete query.skip
        warnings.push('Removed skip because select queries must scan the full result set.')
      }
    }

    const aliases = new Map<string, string>()
    this.collectAliases(query.where, schemaByLabel, aliases)
    this.validateWhere(query.where, schemaByLabel, propertiesByLabel, allProperties, errors, labels)
    this.validateSelectExpressions(query.select, errors)
    this.validateSelectRefs(
      query.select,
      schemaByLabel,
      propertiesByLabel,
      allProperties,
      aliases,
      labels,
      errors,
      labels
    )
    this.validateGroupBy(
      query.groupBy,
      query.select,
      propertiesByLabel,
      allProperties,
      aliases,
      labels,
      errors
    )

    return { query, warnings, errors }
  }

  private selectRequiresFullScan(query: SearchDto): boolean {
    if (!query.select || typeof query.select !== 'object' || Array.isArray(query.select)) {
      return false
    }

    if (Array.isArray(query.groupBy) && query.groupBy.length > 0) {
      return true
    }

    return this.hasFullScanSelectOperator(query.select)
  }

  private hasFullScanSelectOperator(value: unknown): boolean {
    if (!value || typeof value !== 'object') {
      return false
    }
    if (Array.isArray(value)) {
      return value.some((item) => this.hasFullScanSelectOperator(item))
    }

    return Object.entries(value).some(([key, nested]) => {
      if (['$sum', '$avg', '$count', '$min', '$max', '$timeBucket'].includes(key)) {
        return true
      }
      return this.hasFullScanSelectOperator(nested)
    })
  }

  private normalizeOrderBy(query: SearchDto, warnings: string[]) {
    const orderBy = query.orderBy as unknown
    if (
      orderBy &&
      typeof orderBy === 'object' &&
      !Array.isArray(orderBy) &&
      typeof (orderBy as any).property === 'string' &&
      ((orderBy as any).direction === 'asc' || (orderBy as any).direction === 'desc')
    ) {
      query.orderBy = { [(orderBy as any).property]: (orderBy as any).direction }
      warnings.push('Normalized orderBy from property/direction form.')
    }
  }

  // Unbounded hops are rejected by the parser on capped connections; defaulting max to the
  // deployment cap (like the limit/skip normalization above) saves a repair round.
  private normalizeTraversalHops(value: unknown, warnings: string[]) {
    if (!value || typeof value !== 'object') {
      return
    }
    if (Array.isArray(value)) {
      value.forEach((item) => this.normalizeTraversalHops(item, warnings))
      return
    }

    for (const [key, next] of Object.entries(value)) {
      // The $cycle operator's value is a relation spec itself — its hops live directly
      // under the operator, not under a $relation wrapper.
      if (
        (key === '$relation' || key === '$cycle') &&
        next &&
        typeof next === 'object' &&
        !Array.isArray(next)
      ) {
        const hops = (next as any).hops
        if (hops && typeof hops === 'object' && !Array.isArray(hops) && hops.max === undefined) {
          const maxHops = resolveMaxTraversalHops()
          if (maxHops !== Infinity) {
            hops.max = maxHops
            warnings.push(
              `Set hops.max to ${maxHops} because unbounded traversal is not allowed on this connection.`
            )
          }
        }
        continue
      }
      this.normalizeTraversalHops(next, warnings)
    }
  }

  private validateSelectExpressions(select: unknown, errors: string[]) {
    if (!select || typeof select !== 'object' || Array.isArray(select)) {
      return
    }

    for (const [outputKey, expr] of Object.entries(select)) {
      if (!this.isValidSelectExpr(expr)) {
        errors.push(`Invalid select expression for "${outputKey}"`)
      }
    }
  }

  private isValidSelectExpr(expr: unknown): boolean {
    if (typeof expr === 'string' || typeof expr === 'number' || typeof expr === 'boolean') {
      return true
    }
    if (!expr || typeof expr !== 'object' || Array.isArray(expr)) {
      return false
    }

    const keys = Object.keys(expr)
    const operatorKeys = keys.filter((key) => SELECT_OPERATOR_KEYS.has(key))
    if (operatorKeys.length !== 1) {
      return false
    }

    for (const [key, value] of Object.entries(expr)) {
      if (key === '$precision' || key === 'unit' || key === 'size' || key === 'field') {
        continue
      }
      if (key === '$collect' || key === '$timeBucket') {
        return Boolean(value && typeof value === 'object')
      }
      if (key === '$ref') {
        return typeof value === 'string'
      }
      if (['$divide', '$multiply', '$add', '$subtract'].includes(key)) {
        return (
          Array.isArray(value) && value.length === 2 && value.every((item) => this.isValidSelectExpr(item))
        )
      }
      if (SELECT_OPERATOR_KEYS.has(key)) {
        return this.isValidSelectExpr(value)
      }
    }

    return false
  }

  private validateWhere(
    value: unknown,
    schemaByLabel: Map<string, SchemaItem>,
    propertiesByLabel: Map<string, Set<string>>,
    allProperties: Set<string>,
    errors: string[],
    currentLabels: string[] = []
  ) {
    if (!value || typeof value !== 'object') {
      return
    }
    if (Array.isArray(value)) {
      for (const nestedValue of value) {
        this.validateWhere(
          nestedValue,
          schemaByLabel,
          propertiesByLabel,
          allProperties,
          errors,
          currentLabels
        )
      }
      return
    }

    for (const [key, next] of Object.entries(value)) {
      if (LOGICAL_KEYS.has(key)) {
        this.validateWhere(next, schemaByLabel, propertiesByLabel, allProperties, errors, currentLabels)
        continue
      }
      if (key === '$relation') {
        this.validateRelation(next, 1, `label "${currentLabels.join('|') || '<root>'}"`, errors)
        continue
      }
      if (key === '$cycle') {
        this.validateCycleOperator(next, errors)
        continue
      }
      if (COMPARISON_KEYS.has(key) || RELATION_META_KEYS.has(key)) {
        continue
      }
      if (key.startsWith('$')) {
        if (key === '$ref') {
          errors.push(
            `Unsupported where operator "$ref": "$ref" is only valid inside select expressions. ` +
              `where values must be literals — correlated joins between records are not supported. ` +
              `To rank or relate records by a scalar field, root on the label that owns that field and use groupBy.`
          )
        } else {
          errors.push(`Unsupported where operator "${key}"`)
        }
        continue
      }
      if (schemaByLabel.has(key)) {
        this.validateWhere(next, schemaByLabel, propertiesByLabel, allProperties, errors, [key])
        continue
      }
      // `key` is a property predicate — its value must be a literal, not a field/alias reference.
      this.flagFieldRefValues(key, next, errors)
      if (currentLabels.length && !this.propertyExistsOnAnyLabel(key, currentLabels, propertiesByLabel)) {
        errors.push(`Property "${key}" is not available on label ${currentLabels.join('|')}`)
        continue
      }
      if (!currentLabels.length && !allProperties.has(key)) {
        errors.push(`Unknown property or relationship label "${key}"`)
      }
      this.validateWhere(next, schemaByLabel, propertiesByLabel, allProperties, errors, currentLabels)
    }
  }

  // Canonical operator form: { "$cycle": { "type"?, "direction", "hops" } } — the value
  // IS the traversal spec. Mirrors the parser's normalizeHops rules (hops mandatory,
  // floor 2) so a query the spec prompt teaches is not rejected here or at execution.
  private validateCycleOperator(value: unknown, errors: string[]) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || !('hops' in value)) {
      errors.push(
        `'$cycle' requires a traversal spec with 'hops', e.g. ` +
          `{ "$cycle": { "type": "FLOWS_TO", "direction": "out", "hops": { "min": 2, "max": 6 } } }`
      )
      return
    }
    this.validateRelation(value, 2, `'$cycle'`, errors)
  }

  private validateRelation(relation: unknown, hopsFloor: number, context: string, errors: string[]) {
    if (!relation || typeof relation !== 'object' || Array.isArray(relation)) {
      return
    }

    const direction = (relation as any).direction
    const hops = (relation as any).hops
    if (hops === undefined) {
      return
    }
    if (direction !== undefined && direction !== 'in' && direction !== 'out') {
      errors.push(
        `'$relation.direction' must be "in" or "out" when 'hops' is set in ${context}, got ${JSON.stringify(direction)}`
      )
    }

    const maxHops = resolveMaxTraversalHops()
    const maxHopsLabel = maxHops === Infinity ? 'unbounded' : maxHops
    if (typeof hops === 'number') {
      if (!Number.isInteger(hops) || hops < hopsFloor || hops > maxHops) {
        errors.push(
          `'hops' in ${context} must be an integer between ${hopsFloor} and ${maxHopsLabel}, got ${JSON.stringify(hops)}`
        )
      }
      return
    }
    if (hops && typeof hops === 'object' && !Array.isArray(hops)) {
      const max = (hops as any).max
      const min = (hops as any).min ?? hopsFloor
      if (max === undefined) {
        if (maxHops !== Infinity) {
          errors.push(
            `'hops.max' is required in ${context}: unbounded traversal is not allowed on this connection (max allowed hops: ${maxHops})`
          )
        }
      } else if (!Number.isInteger(max) || max < hopsFloor || max > maxHops) {
        errors.push(
          `'hops.max' in ${context} must be an integer between ${hopsFloor} and ${maxHopsLabel}, got ${JSON.stringify(max)}`
        )
      }
      if (!Number.isInteger(min) || min < hopsFloor || (Number.isInteger(max) && min > max)) {
        errors.push(
          `'hops.min' in ${context} must be an integer between ${hopsFloor} and 'hops.max', got ${JSON.stringify(min)}`
        )
      }
      return
    }
    errors.push(`'hops' in ${context} must be a number or { min?, max? } object, got ${JSON.stringify(hops)}`)
  }

  private validateSelectRefs(
    value: unknown,
    schemaByLabel: Map<string, SchemaItem>,
    propertiesByLabel: Map<string, Set<string>>,
    allProperties: Set<string>,
    aliases: Map<string, string>,
    rootLabels: string[],
    errors: string[],
    currentLabels: string[] = rootLabels
  ) {
    if (!value) {
      return
    }
    if (typeof value === 'string') {
      this.validateFieldRef(value, propertiesByLabel, allProperties, aliases, rootLabels, errors)
      return
    }
    if (Array.isArray(value)) {
      value.forEach((item) =>
        this.validateSelectRefs(
          item,
          schemaByLabel,
          propertiesByLabel,
          allProperties,
          aliases,
          rootLabels,
          errors,
          currentLabels
        )
      )
      return
    }
    if (typeof value !== 'object') {
      return
    }

    const collect = (value as any).$collect
    if (collect?.label && !schemaByLabel.has(collect.label)) {
      errors.push(`Unknown collect label "${collect.label}"`)
    }
    if (
      collect?.label &&
      schemaByLabel.has(collect.label) &&
      currentLabels.length &&
      !this.hasDirectRelationshipToEveryLabel(currentLabels, collect.label, schemaByLabel)
    ) {
      errors.push(
        `Collect label "${collect.label}" is not directly related to ${currentLabels.join('|')}; use where + $alias and $collect.from for intermediate paths.`
      )
    }
    if (collect?.from && typeof collect.from === 'string' && !aliases.has(collect.from)) {
      errors.push(`Unknown collect alias "${collect.from}"`)
    }

    const nestedLabels =
      collect?.label && typeof collect.label === 'string' && schemaByLabel.has(collect.label) ?
        [collect.label]
      : currentLabels

    for (const nested of Object.values(value)) {
      this.validateSelectRefs(
        nested,
        schemaByLabel,
        propertiesByLabel,
        allProperties,
        aliases,
        rootLabels,
        errors,
        nestedLabels
      )
    }
  }

  private validateGroupBy(
    groupBy: unknown,
    select: unknown,
    propertiesByLabel: Map<string, Set<string>>,
    allProperties: Set<string>,
    aliases: Map<string, string>,
    rootLabels: string[],
    errors: string[]
  ) {
    if (!Array.isArray(groupBy)) {
      return
    }
    for (const ref of groupBy) {
      if (typeof ref === 'string') {
        if (ref.startsWith('$') && !ref.includes('.')) {
          errors.push(`groupBy field reference "${ref}" must include a property, e.g. "${ref}.name"`)
          continue
        }
        if (!ref.startsWith('$')) {
          if (!select || typeof select !== 'object' || Array.isArray(select) || !(ref in select)) {
            errors.push(`groupBy key "${ref}" must reference a key defined in select`)
          }
          continue
        }
        this.validateFieldRef(ref, propertiesByLabel, allProperties, aliases, rootLabels, errors)
      }
    }
  }

  // Flags where-predicate values that look like field/alias references (e.g. "$record.id"). Such a value is a
  // correlated-join attempt, which the where grammar does not support — it is matched as a literal and returns
  // nothing. Recurses into comparison operators ($eq, $in, …) so wrapped forms are caught too.
  private flagFieldRefValues(propertyKey: string, payload: unknown, errors: string[]) {
    const inspect = (candidate: unknown) => {
      if (typeof candidate === 'string') {
        if (FIELD_REF_VALUE_REGEX.test(candidate)) {
          errors.push(
            `where value "${candidate}" for "${propertyKey}" is a field/alias reference, which is not supported ` +
              `in where (it is matched as a literal string). Field references such as $record.* or $alias.* are ` +
              `only valid in select, groupBy, and aggregate. To rank or relate records by a field, root on the ` +
              `label that owns it and use groupBy, or traverse an existing relationship from the schema.`
          )
        }
        return
      }
      if (Array.isArray(candidate)) {
        candidate.forEach(inspect)
        return
      }
      if (candidate && typeof candidate === 'object') {
        for (const [innerKey, innerValue] of Object.entries(candidate)) {
          if (COMPARISON_KEYS.has(innerKey)) {
            inspect(innerValue)
          }
        }
      }
    }
    inspect(payload)
  }

  private validateFieldRef(
    ref: string,
    propertiesByLabel: Map<string, Set<string>>,
    allProperties: Set<string>,
    aliases: Map<string, string>,
    rootLabels: string[],
    errors: string[]
  ) {
    if (!ref.startsWith('$')) {
      return
    }
    if (ref === '$record' || ref === '$self' || ref === '*') {
      return
    }

    const [alias, property] = ref.split('.')
    if (!property || property.startsWith('$')) {
      return
    }
    if (alias === '$record') {
      if (rootLabels.length && !this.propertyExistsOnAnyLabel(property, rootLabels, propertiesByLabel)) {
        errors.push(`Field reference "${ref}" is not available on root label ${rootLabels.join('|')}`)
      } else if (!rootLabels.length && !allProperties.has(property)) {
        errors.push(`Unknown field reference "${ref}"`)
      }
      return
    }

    if (alias === '$self') {
      return
    }

    const label = aliases.get(alias)
    if (!label) {
      errors.push(`Unknown alias reference "${alias}"`)
      return
    }
    if (!propertiesByLabel.get(label)?.has(property)) {
      errors.push(`Field reference "${ref}" is not available on label ${label}`)
    }
  }

  private collectAliases(
    value: unknown,
    schemaByLabel: Map<string, SchemaItem>,
    aliases: Map<string, string>,
    currentLabel?: string
  ) {
    if (!value || typeof value !== 'object') {
      return
    }
    if (Array.isArray(value)) {
      value.forEach((item) => this.collectAliases(item, schemaByLabel, aliases, currentLabel))
      return
    }

    for (const [key, next] of Object.entries(value)) {
      if (key === '$alias' && typeof next === 'string' && currentLabel) {
        aliases.set(next, currentLabel)
        continue
      }
      if (schemaByLabel.has(key)) {
        this.collectAliases(next, schemaByLabel, aliases, key)
        continue
      }
      this.collectAliases(next, schemaByLabel, aliases, currentLabel)
    }
  }

  private propertyExistsOnAnyLabel(
    property: string,
    labels: string[],
    propertiesByLabel: Map<string, Set<string>>
  ) {
    return labels.some((label) => propertiesByLabel.get(label)?.has(property))
  }

  private hasDirectRelationshipToEveryLabel(
    sourceLabels: string[],
    targetLabel: string,
    schemaByLabel: Map<string, SchemaItem>
  ) {
    return sourceLabels.every((sourceLabel) =>
      schemaByLabel.get(sourceLabel)?.relationships.some((relationship) => relationship.label === targetLabel)
    )
  }
}
