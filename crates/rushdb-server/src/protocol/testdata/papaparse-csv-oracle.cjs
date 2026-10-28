// Reference outputs for the engine's CSV decoding and export (protocol/csv.rs).
//
// This is the platform's CSV import (Papa Parse 5.4.1 `parse`, then the header
// clean-up of import.controller.ts) and CSV export (`unparse` of export.controller.ts)
// as they ran before CSV moved into the engine. csv_tests.rs asserts that the engine
// reproduces every case. Regenerate after adding a case:
//
//   node papaparse-csv-oracle.cjs /path/to/node_modules/papaparse > papaparse-csv-oracle.json
const { parse, unparse } = require(process.argv[2] ?? 'papaparse')
function coreCsv(data, options = {}, parseConfig = {}) {
  const defaultConfig = { header: true, delimiter: ',', skipEmptyLines: true, dynamicTyping: options.suggestTypes ?? false }
  const config = Object.assign({}, defaultConfig, parseConfig, {
    dynamicTyping: typeof parseConfig.dynamicTyping === 'boolean' ? parseConfig.dynamicTyping : defaultConfig.dynamicTyping
  })
  const result = parse(data, config)
  if (result.errors?.length) return { error: `CSV parse error: ${result.errors[0].message}` }
  const originalFields = result?.meta?.fields || []
  const seen = new Set(); const fieldReplacements = {}
  originalFields.forEach((raw, idx) => {
    let name = (raw ?? '').replace(/\0/g, '').trim()
    if (!name) name = `column_${idx + 1}`
    name = name.replace(/\s+/g, '_')
    if (!/^[A-Za-z_]/.test(name)) name = `_${name}`
    if (seen.has(name)) { const base = name; let c = 2; while (seen.has(`${base}_${c}`)) c++; name = `${base}_${c}` }
    seen.add(name); fieldReplacements[raw] = name
  })
  const rows = result.data.map((draft) => {
    if (draft && typeof draft === 'object' && !Array.isArray(draft)) {
      return Object.fromEntries(Object.entries(draft).filter(([k, v]) => v !== null && v !== undefined && k !== '').map(([k, v]) => [fieldReplacements[k] ?? k, v]).filter(([k]) => k !== ''))
    }
    if (Array.isArray(draft)) return draft.reduce((acc, val, i) => { acc[`column_${i + 1}`] = val; return acc }, {})
    return {}
  })
  return { rows: JSON.parse(JSON.stringify(rows)) }
}
const cases = [
  ['basic', 'name,age\nAda,36\nAlan,41\n', {}, {}],
  ['typed', 'name,age,active,score,when,empty\nAda,36,true,1.50,2024-01-02T03:04:05Z,\nBob,007,FALSE,-2e3,2024-01-02,x\n', { suggestTypes: true }, {}],
  ['typed-edge', 'a,b,c,d,e\n 12 ,1e400,.5,5.,9007199254740993\nTrue,tRuE,null,NaN,0x10\n', {}, { dynamicTyping: true }],
  ['dates', 'a,b,c\n2024-13-01T00:00:00Z,2024-01-02T03:04:05.123+02:00,2024-01-02T03:04Z\n', {}, { dynamicTyping: true }],
  ['quoted', 'name,note\n"Smith, J","He said ""hi"""\n"multi\nline",x\n', {}, {}],
  ['crlf', 'a,b\r\n1,2\r\n3,4\r\n', {}, {}],
  ['cr', 'a,b\r1,2\r3,4', {}, {}],
  ['semicolon', 'a;b\n1;2\n', {}, { delimiter: ';' }],
  ['multichar-delim', 'a||b\n1||2\n', {}, { delimiter: '||' }],
  ['tab', 'a\tb\n1\t2\n', {}, { delimiter: '\t' }],
  ['headers', ' first name ,,9lives,a\u0000b,first_name,name,name\n1,2,3,4,5,6,7\n', {}, {}],
  ['empty-values', 'a,b,c\n1,,3\n,,\n', {}, {}],
  ['empty-values-typed', 'a,b,c\n1,,3\n', {}, { dynamicTyping: true }],
  ['blank-lines', 'a,b\n\n1,2\n\n\n3,4\n', {}, {}],
  ['blank-lines-off', 'a,b\n1,2\n\n3,4\n', {}, { skipEmptyLines: false }],
  ['greedy', 'a,b\n1,2\n , \n3,4\n', {}, { skipEmptyLines: 'greedy' }],
  ['greedy-off-ws', 'a,b\n1,2\n , \n', {}, {}],
  ['too-few', 'a,b,c\n1,2\n', {}, {}],
  ['too-many', 'a,b\n1,2,3\n', {}, {}],
  ['no-header', '1,2\n3,4\n', {}, { header: false }],
  ['no-header-typed', '1,,x\n', {}, { header: false, dynamicTyping: true }],
  ['quote-char', "a,b\n'x,y',z\n", {}, { quoteChar: "'" }],
  ['escape-char', 'a,b\n"x\\"y",z\n', {}, { escapeChar: '\\' }],
  ['newline-explicit', 'a,b|1,2|3,4', {}, { newline: '|' }],
  ['header-only', 'a,b\n', {}, {}],
  ['empty', '', {}, {}],
  ['trailing-no-newline', 'a,b\n1,2', {}, {}],
  ['unterminated-quote', 'a,b\n"1,2\n', {}, {}],
  ['bom', '﻿a,b\n1,2\n', {}, {}],
  ['spaces', 'a,b\n x , y \n', {}, {}],
  ['dup-header', 'a,a,a_1\n1,2,3\n', {}, {}],
  ['quote-mid-field', 'a,b\nx"y,z\n', {}, {}],
  ['quoted-then-text', 'a,b\n"x"y,z\n', {}, {}],
  ['trailing-newline-noskip', 'a,b\n1,2\n', {}, { skipEmptyLines: false }],
  ['greedy-commas', 'a,b\n1,2\n,\n', {}, { skipEmptyLines: 'greedy' }],
  ['commas-default', 'a,b\n1,2\n,\n', {}, {}],
  ['leading-blank', '\n\na,b\n1,2\n', {}, {}],
  ['no-header-blank', '1,2\n\n3\n', {}, { header: false }],
  ['no-header-noskip', '1,2\n\n3\n', {}, { header: false, skipEmptyLines: false }],
  ['quote-space-delim', 'a,b\n"x" ,y\n', {}, {}],
  ['space-before-quote', 'a,b\n "x",y\n', {}, {}],
  ['escape-custom-doubled', 'a,b\n"x""y",z\n', {}, { escapeChar: '\\' }],
  ['quoted-header', '"a,1",b\n1,2\n', {}, {}],
  ['quoted-empty', 'a,b\n"",x\n', {}, { dynamicTyping: true }],
  ['typed-neg-zero', 'a,b,c,d\n-0,-.5,1E3,+1\n', {}, { dynamicTyping: true }],
  ['typed-ws-bool', 'a,b\n true,TRUE\n', {}, { dynamicTyping: true }],
  ['typed-quoted-number', 'a\n"42"\n', {}, { dynamicTyping: true }],
  ['mixed-newlines', 'a,b\r\n1,2\n3,4\r\n', {}, {}],
  ['cr-in-quotes', 'a,b\n"x\ry",z\n', {}, {}],
  ['unicode', 'имя,年齢\nАда,三十\n', {}, {}],
  ['unicode-header-prefix', 'é,1a\nx,y\n', {}, {}],
  ['empty-header-cell', 'a,,c\n1,2,3\n', {}, {}],
  ['only-newlines', '\n\n\n', {}, {}],
  ['delim-in-unquoted-header-dup', 'x,x,x\n1,2,3\n', {}, {}],
  ['many-dups', 'a,a,a,a_2\n1,2,3,4\n', {}, {}],
  ['ws-header', 'a b\tc,d\n1,2\n', {}, {}],
  ['dates-edge', 'a,b,c,d,e,f,g,h\n2024-02-30T00:00:00Z,2024-01-01T24:00:00Z,2024-01-01T24:00:01Z,2024-01-01T23:59:59+29:59,2024-01-01T00:00:00.123456Z,0000-01-01T00:00:00Z,2024-04-31T10:00+05:30,2024-01-01T00:00:00.9Z\n', {}, { dynamicTyping: true }],
  ['typed-range', 'a,b,c,d\n9007199254740991,-9007199254740991,9007199254740992,1.7976931348623157e308\n', {}, { dynamicTyping: true }],
  ['typed-floats', 'a,b,c,d\n0.1,1e-7,123456789.123,3.0e0\n', {}, { dynamicTyping: true }],
]
const out = {}
for (const [name, data, options, parseConfig] of cases) out[name] = { input: { data, options, parseConfig }, ...coreCsv(data, options, parseConfig) }
// Export oracle: Core's export controller over records already shaped as Core rows.
const exportRows = [
  { __id: 'id-1', __label: 'Book', title: 'Dune, part 1', pages: 412, tags: ['a', 'b'], rating: 4.5, big: 1e21, small: 1e-7, active: true, note: ' padded ', quote: 'say "hi"', nl: 'two\nlines' },
  { __id: 'id-2', __label: 'Book', title: 'Plain', extra: null }
]
function coreExport(data) {
  const fieldSet = new Set(); for (const row of data) Object.keys(row || {}).forEach((k) => fieldSet.add(k))
  const first = ['__id'], last = ['__label']
  const others = Array.from(fieldSet).filter((k) => !first.includes(k) && !last.includes(k)).sort()
  const fields = [...first.filter((f) => fieldSet.has(f)), ...last.filter((l) => fieldSet.has(l)), ...others]
  const rows = data.map((row) => fields.map((f) => row?.[f] ?? ''))
  return unparse({ fields, data: rows })
}
out.__export = { rows: exportRows, csv: coreExport(exportRows), empty: coreExport([]) }
console.log(JSON.stringify(out, null, 1))
