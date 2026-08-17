/**
 * Rust binding generator for the private Cloud destination contract (v1).
 *
 * Reads `schema/*.json`, emits `src/generated.rs` (serde types + schema hash
 * constants) into the private `rushdb-synx` `synx-contract` crate, and mirrors
 * the schemas and fixtures into that crate's `resources/` directory so its
 * conformance tests run against the exact same artifacts.
 *
 * Supported JSON Schema subset (validated; anything else fails loudly):
 *   object (+ properties/required/additionalProperties), string, integer,
 *   boolean, array (+ items), enum (named via `title`, deduped globally),
 *   const, $ref (local `#/$defs/<Name>`), oneOf (discriminated union via a
 *   `type` const on each branch), and `type: object` without properties
 *   (=> `serde_json::Value`).
 *
 * Usage:
 *   pnpm generate:rust            # write into ../../rushdb-synx/crates/synx-contract
 *   pnpm generate:rust --out <dir>
 *   pnpm generate:rust --check    # verify committed output is up to date
 */

import { execFileSync } from 'node:child_process'
import { mkdir, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { join, dirname, resolve, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

import { computeSchemaHash } from '../src/hash'
import { CONTRACT_VERSION } from '../src/version'
import { SCHEMAS_V1 } from '../src/schemas'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const defaultOut = resolve(root, '..', '..', '..', 'rushdb-synx', 'crates', 'synx-contract')

const args = process.argv.slice(2)
const checkOnly = args.includes('--check')
const outFlag = args.indexOf('--out')
const outDir = resolve(outFlag >= 0 ? args[outFlag + 1] : defaultOut)

if (!outFlag && outDir !== defaultOut) {
  console.error(`refusing unexpected default out dir: ${outDir}`)
  process.exit(1)
}

type Json = Record<string, unknown>

function assert(cond: unknown, message: string): asserts cond {
  if (!cond) {
    throw new Error(`generate-rust: ${message}`)
  }
}

function asObj(value: unknown): Json {
  assert(value !== null && typeof value === 'object' && !Array.isArray(value), 'expected object schema node')
  return value as Json
}

function asArray(value: unknown): unknown[] {
  assert(Array.isArray(value), 'expected array')
  return value as unknown[]
}

function str(value: unknown): string {
  assert(typeof value === 'string', 'expected string')
  return value
}

function camelToSnake(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`)
}

const RUST_KEYWORDS = new Set([
  'abstract',
  'as',
  'async',
  'await',
  'become',
  'box',
  'break',
  'const',
  'continue',
  'crate',
  'do',
  'dyn',
  'else',
  'enum',
  'extern',
  'false',
  'final',
  'fn',
  'for',
  'gen',
  'if',
  'impl',
  'in',
  'let',
  'loop',
  'macro',
  'match',
  'mod',
  'move',
  'mut',
  'override',
  'priv',
  'pub',
  'ref',
  'return',
  'self',
  'Self',
  'static',
  'struct',
  'super',
  'trait',
  'true',
  'try',
  'type',
  'typeof',
  'unsafe',
  'unsized',
  'use',
  'virtual',
  'where',
  'while',
  'yield'
])

/** Escape Rust keywords used as field identifiers via raw identifiers. */
function rustFieldName(snake: string): string {
  return RUST_KEYWORDS.has(snake) ? `r#${snake}` : snake
}

function toPascal(value: string): string {
  return value
    .split(/[_\s-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('')
}

interface StructField {
  json: string
  rust: string
  schema: Json
  optional: boolean
}

interface StructKind {
  kind: 'struct'
  schema: Json
  required: string[]
  fields: StructField[]
}

interface EnumKind {
  kind: 'enum'
  values: string[]
}

interface UnionVariant {
  rust: string
  tag: string
  fields: StructField[]
}

interface UnionKind {
  kind: 'union'
  variants: UnionVariant[]
}

type TypeKind = StructKind | EnumKind | UnionKind

class Registry {
  types = new Map<string, TypeKind>()
  pending: string[] = []
  unionBranchDefs = new Set<string>()

  has(name: string): boolean {
    return this.types.has(name)
  }

  register(name: string, kind: TypeKind): void {
    const existing = this.types.get(name)
    if (existing) {
      assert(sameKind(existing, kind), `duplicate type name with different shape: ${name}`)
      return
    }
    this.types.set(name, kind)
  }

  deferDef(name: string): void {
    if (!this.pending.includes(name)) {
      this.pending.push(name)
    }
  }
}

function arraysEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i])
}

function sameKind(a: TypeKind, b: TypeKind): boolean {
  if (a.kind !== b.kind) {
    return false
  }
  if (a.kind === 'enum' && b.kind === 'enum') {
    return arraysEqual(a.values, b.values)
  }
  if (a.kind === 'struct' && b.kind === 'struct') {
    return (
      arraysEqual(a.required, b.required) &&
      a.fields.length === b.fields.length &&
      a.fields.every((f, i) => f.json === b.fields[i].json && f.optional === b.fields[i].optional)
    )
  }
  if (a.kind === 'union' && b.kind === 'union') {
    return (
      a.variants.length === b.variants.length &&
      a.variants.every(
        (v, i) =>
          v.rust === b.variants[i].rust &&
          arraysEqual(
            v.fields.map((f) => f.json),
            b.variants[i].fields.map((f) => f.json)
          )
      )
    )
  }
  return false
}

/** Queue every `$ref` found anywhere in `schema` for deferred registration. */
function queueRefs(reg: Registry, schema: unknown): void {
  if (Array.isArray(schema)) {
    schema.forEach((node) => queueRefs(reg, node))
    return
  }
  if (schema !== null && typeof schema === 'object') {
    const record = schema as Json
    if (typeof record.$ref === 'string') {
      const match = /^#\/\$defs\/(.+)$/.exec(record.$ref)
      if (match) {
        reg.deferDef(match[1])
      }
    }
    for (const value of Object.values(record)) {
      queueRefs(reg, value)
    }
  }
}

/**
 * Resolve a property/items schema to a Rust type name, registering named types
 * (enums, unions) as needed.
 */
function resolveType(reg: Registry, defs: Record<string, unknown>, path: string, schema: Json): string {
  if (typeof schema.$ref === 'string') {
    const match = /^#\/\$defs\/(.+)$/.exec(schema.$ref)
    assert(match, `unsupported $ref: ${schema.$ref}`)
    return match[1]
  }
  if (Array.isArray(schema.enum)) {
    const values = schema.enum.map((v) => str(v))
    assert(typeof schema.title === 'string', `inline enum without a title (use items.title): ${path}`)
    const name = schema.title
    reg.register(name, { kind: 'enum', values })
    return name
  }
  if (schema.const !== undefined) {
    return 'String'
  }
  if (Array.isArray(schema.oneOf)) {
    return resolveUnion(
      reg,
      defs,
      path,
      schema.oneOf.map((b) => asObj(b))
    )
  }
  switch (schema.type) {
    case 'string':
      return 'String'
    case 'integer':
      return 'i64'
    case 'number':
      return 'f64'
    case 'boolean':
      return 'bool'
    case 'object':
      return 'serde_json::Value'
    case 'array': {
      assert(schema.items, `array without items: ${path}`)
      const inner = resolveType(reg, defs, `${path}Item`, asObj(schema.items))
      return `Vec<${inner}>`
    }
    default:
      assert(false, `unsupported schema for ${path}: ${JSON.stringify(schema.type)}`)
  }
}

function fieldsOf(required: string[] | undefined, properties: Json): StructField[] {
  const requiredSet = new Set(required ?? [])
  return Object.entries(properties).map(([json, propSchema]) => ({
    json,
    rust: rustFieldName(camelToSnake(json)),
    schema: asObj(propSchema),
    optional: !requiredSet.has(json)
  }))
}

/** Resolve a discriminated union (each branch carries a `type` const). */
function resolveUnion(reg: Registry, defs: Record<string, unknown>, path: string, branches: Json[]): string {
  const unionName = str(branches[0].title ?? path)
  if (reg.has(unionName)) {
    return unionName
  }
  const variants: UnionVariant[] = branches.map((branch) => {
    let resolved = branch
    if (typeof branch.$ref === 'string') {
      const refName = branch.$ref.replace(/^#\/\$defs\//, '')
      resolved = asObj(defs[refName])
      reg.unionBranchDefs.add(refName)
    }
    assert(resolved, `union branch must resolve: ${JSON.stringify(branch)}`)
    const props = asObj(resolved.properties)
    const typeField = asObj(props.type)
    assert(typeof typeField.const === 'string', `union branch must have a string 'type' const: ${path}`)
    const tag = typeField.const
    const branchRequired = (resolved.required as string[] | undefined) ?? []
    const fields = fieldsOf(
      branchRequired,
      Object.fromEntries(Object.entries(props).filter(([json]) => json !== 'type'))
    )
    return { rust: toPascal(tag), tag, fields }
  })
  reg.register(unionName, { kind: 'union', variants })
  for (const branch of branches) {
    queueRefs(reg, branch)
  }
  return unionName
}

/** Register a struct type (root or `$defs` object) and return its name. */
function registerStruct(reg: Registry, defs: Record<string, unknown>, name: string, schema: Json): string {
  if (reg.has(name)) {
    return name
  }
  const required = (schema.required as string[] | undefined) ?? []
  const fields = fieldsOf(required, asObj(schema.properties))
  reg.register(name, { kind: 'struct', schema, required, fields })
  queueRefs(reg, schema)
  return name
}

/** Process pending `$defs` targets until the registry is stable. */
function drainDefs(reg: Registry, defs: Record<string, unknown>): void {
  while (reg.pending.length > 0) {
    const name = reg.pending.shift() as string
    if (reg.has(name)) {
      continue
    }
    const def = asObj(defs[name])
    assert(def, `$defs entry missing: ${name}`)
    queueRefs(reg, def)
    if (Array.isArray(def.oneOf)) {
      resolveUnion(
        reg,
        defs,
        name,
        def.oneOf.map((b) => asObj(b))
      )
    } else if (Array.isArray(def.enum)) {
      reg.register(name, { kind: 'enum', values: def.enum.map((v) => str(v)) })
    } else {
      registerStruct(reg, defs, name, def)
    }
  }
}

function buildRegistry(schemas: Record<string, unknown>): { reg: Registry; defs: Record<string, unknown> } {
  const defs: Record<string, unknown> = {}
  for (const raw of Object.values(schemas)) {
    Object.assign(defs, asObj(asObj(raw).$defs ?? {}))
  }
  const reg = new Registry()
  for (const raw of Object.values(schemas)) {
    const schema = asObj(raw)
    registerStruct(reg, defs, str(schema.title), schema)
  }
  drainDefs(reg, defs)
  return { reg, defs }
}

function docLines(schema: Json, indent: string): string[] {
  const parts: string[] = []
  if (typeof schema.title === 'string') {
    parts.push(schema.title)
  }
  if (typeof schema.description === 'string') {
    parts.push(schema.description)
  }
  if (schema.const !== undefined) {
    parts.push(`Must equal "${String(schema.const)}".`)
  }
  return parts.map((line) => `${indent}/// ${line}`)
}

function fieldType(reg: Registry, defs: Record<string, unknown>, path: string, field: StructField): string {
  const ty = resolveType(reg, defs, `${path}.${field.json}`, field.schema)
  return field.optional ? `Option<${ty}>` : ty
}

function emitEnum(name: string, kind: EnumKind): string {
  const variants = kind.values.map((v) => `    ${toPascal(v)},`).join('\n')
  return [
    '#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]',
    '#[serde(rename_all = "snake_case")]',
    `pub enum ${name} {`,
    variants,
    '}'
  ].join('\n')
}

function emitUnion(reg: Registry, defs: Record<string, unknown>, name: string, kind: UnionKind): string {
  const variants = kind.variants
    .map((variant) => {
      const fields = variant.fields
        .map((field) => {
          const ty = fieldType(reg, defs, `${name}.${variant.rust}`, field)
          const docs = docLines(field.schema, '        ')
          const attrs: string[] = []
          if (field.optional) {
            attrs.push('        #[serde(skip_serializing_if = "Option::is_none")]')
          }
          attrs.push(`        ${field.rust}: ${ty},`)
          return [...docs, ...attrs].join('\n')
        })
        .join('\n')
      return `    ${variant.rust} {\n${fields}\n    },`
    })
    .join('\n')
  return [
    '/// Discriminated union tagged by `type`.',
    '#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]',
    '#[allow(clippy::large_enum_variant)]',
    '#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]',
    `pub enum ${name} {`,
    variants,
    '}'
  ].join('\n')
}

function emitStruct(reg: Registry, defs: Record<string, unknown>, name: string, kind: StructKind): string {
  const lines: string[] = [...docLines(kind.schema, '')]
  lines.push('#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]')
  lines.push('#[serde(rename_all = "camelCase")]')
  lines.push(`pub struct ${name} {`)
  for (const field of kind.fields) {
    const ty = fieldType(reg, defs, name, field)
    lines.push(...docLines(field.schema, '    '))
    if (field.optional) {
      lines.push('    #[serde(skip_serializing_if = "Option::is_none")]')
    }
    lines.push(`    pub ${field.rust}: ${ty},`)
  }
  lines.push('}')
  return lines.join('\n')
}

function emitAll(schemas: Record<string, unknown>, schemaHash: string): string {
  const { reg, defs } = buildRegistry(schemas)

  // Pre-pass: resolve every field type so inline enums/unions register before partitioning.
  for (const [name, kind] of reg.types) {
    if (kind.kind === 'struct') {
      for (const field of kind.fields) {
        fieldType(reg, defs, name, field)
      }
    }
    if (kind.kind === 'union') {
      for (const variant of kind.variants) {
        for (const field of variant.fields) {
          fieldType(reg, defs, `${name}.${variant.rust}`, field)
        }
      }
    }
  }

  const byName = (a: string, b: string) => a.localeCompare(b)
  const enums = [...reg.types.entries()].filter(([, k]) => k.kind === 'enum').sort(([a], [b]) => byName(a, b))
  const unions = [...reg.types.entries()]
    .filter(([, k]) => k.kind === 'union')
    .sort(([a], [b]) => byName(a, b))
  const structs = [...reg.types.entries()]
    .filter(([name, k]) => k.kind === 'struct' && !reg.unionBranchDefs.has(name))
    .sort(([a], [b]) => byName(a, b))

  const blocks: string[] = []
  for (const [name, kind] of enums) blocks.push(emitEnum(name, kind as EnumKind))
  for (const [name, kind] of unions) blocks.push(emitUnion(reg, defs, name, kind as UnionKind))
  for (const [name, kind] of structs) blocks.push(emitStruct(reg, defs, name, kind as StructKind))

  return [
    `// @generated by packages/synx-contract/scripts/generate-rust.ts — DO NOT EDIT`,
    `// Schema set hash: ${schemaHash}`,
    '',
    `use serde::{Deserialize, Serialize};`,
    '',
    `pub const CONTRACT_VERSION: &str = "${CONTRACT_VERSION}";`,
    `pub const SCHEMA_HASH_V1: &str = "${schemaHash}";`,
    '',
    ...blocks,
    ''
  ].join('\n')
}

/**
 * Run `rustfmt` on a generated file in place so the committed output stays
 * `cargo fmt --check`-clean. Swallows failures (e.g. rustfmt not installed) but
 * warns loudly, since unformatted output will break the Synx CI fmt gate.
 */
function formatRustFile(file: string): void {
  try {
    execFileSync('rustfmt', ['--edition', '2021', file], { stdio: 'pipe' })
  } catch (error) {
    console.warn(`warning: rustfmt unavailable; generated file may not be \`cargo fmt\`-clean: ${file}`)
    if (process.env.DEBUG) {
      console.warn(error instanceof Error ? error.message : error)
    }
  }
}

async function copyDir(srcDir: string, destDir: string): Promise<void> {
  for (const entry of await readdir(srcDir)) {
    const src = join(srcDir, entry)
    const dest = join(destDir, entry)
    const info = await stat(src)
    if (info.isDirectory()) {
      await copyDir(src, dest)
    } else {
      await mkdir(destDir, { recursive: true })
      await writeFile(dest, await readFile(src))
    }
  }
}

async function compareTree(a: string, b: string): Promise<string[]> {
  const diffs: string[] = []
  const walk = async (dirA: string, dirB: string): Promise<void> => {
    const rel = relative(a, dirA)
    for (const entry of await readdir(dirA)) {
      const fullA = join(dirA, entry)
      const fullB = join(dirB, entry)
      const relPath = rel === '' ? entry : `${rel}${sep}${entry}`
      const statA = await stat(fullA)
      const statB = await stat(fullB).catch(() => null)
      if (!statB) {
        diffs.push(`missing in target: ${relPath}`)
        continue
      }
      if (statA.isDirectory() !== statB.isDirectory()) {
        diffs.push(`type mismatch: ${relPath}`)
        continue
      }
      if (statA.isDirectory()) {
        await walk(fullA, fullB)
        continue
      }
      const contentA = await readFile(fullA, 'utf8')
      const contentB = await readFile(fullB, 'utf8')
      if (contentA !== contentB) {
        diffs.push(`differs: ${relPath}`)
      }
    }
  }
  await walk(a, b)
  return diffs
}

async function main(): Promise<void> {
  const schemaHash = computeSchemaHash(SCHEMAS_V1)
  const generated = emitAll(SCHEMAS_V1, schemaHash)

  const srcDir = join(outDir, 'src')
  const resourcesDir = join(outDir, 'resources')
  const tmpDir = join(root, '.rust-gen')

  await rm(tmpDir, { recursive: true, force: true })
  await mkdir(join(tmpDir, 'src'), { recursive: true })
  await writeFile(join(tmpDir, 'src', 'generated.rs'), generated)
  await copyDir(join(root, 'schema'), join(tmpDir, 'resources', 'schema'))
  await copyDir(join(root, 'fixtures'), join(tmpDir, 'resources', 'fixtures'))
  await writeFile(join(tmpDir, 'src', 'schema-hash.txt'), `${schemaHash}\n`)
  formatRustFile(join(tmpDir, 'src', 'generated.rs'))

  if (checkOnly) {
    const srcDiffs = await compareTree(join(tmpDir, 'src'), srcDir).catch(async () => [
      'generated src missing'
    ])
    const resDiffs = await compareTree(join(tmpDir, 'resources'), resourcesDir).catch(async () => [
      'generated resources missing'
    ])
    await rm(tmpDir, { recursive: true, force: true })
    if (srcDiffs.length > 0 || resDiffs.length > 0) {
      console.error(
        `generated Rust output is OUT OF DATE. Run \`pnpm generate:rust\`.\n${[...srcDiffs, ...resDiffs].map((d) => `  - ${d}`).join('\n')}`
      )
      process.exit(1)
    }
    console.log(`generate-rust --check OK (hash=${schemaHash})`)
    return
  }

  await mkdir(srcDir, { recursive: true })
  await mkdir(resourcesDir, { recursive: true })
  await writeFile(join(srcDir, 'generated.rs'), generated)
  formatRustFile(join(srcDir, 'generated.rs'))
  await writeFile(join(srcDir, 'schema-hash.txt'), `${schemaHash}\n`)
  await copyDir(join(tmpDir, 'resources', 'schema'), join(resourcesDir, 'schema'))
  await copyDir(join(tmpDir, 'resources', 'fixtures'), join(resourcesDir, 'fixtures'))
  await rm(tmpDir, { recursive: true, force: true })
  console.log(`generated ${join(srcDir, 'generated.rs')} + ${resourcesDir} (hash=${schemaHash})`)
}

void main()
