// Complete RushDB SearchQuery specification.
// Returned by the getSearchQuerySpec tool so the spec lives outside the system
// prompt and is loaded into context only when needed — keeping the system prompt
// short and letting each LLM call attend to the full reference as a focused tool
// result rather than a distant memory from session start.

export const SEARCH_QUERY_SPEC = `━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
RUSHDB SEARCHQUERY — COMPLETE REFERENCE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

SearchQuery shape:
  labels?    string[]      — filter by record type(s); multi-label = OR
  where?     object        — filter conditions; see §1
  select?    object        — expression-based output shaping; see §2
  groupBy?   string[]      — shapes select output; see §3
  orderBy?   string|object — 'asc'|'desc' or { field:'asc'|'desc' }
  limit?     number        — max root records (default 100; max 1000)
  skip?      number        — pagination offset

labels defines root records only. For relationship-aware analytics, do not put related
labels in labels; put related labels inside where traversal blocks and add $alias when
they are referenced by select or groupBy.

For "which/what <parent> has most/more/least/less/fewer/fewest <related records>"
questions, choose the parent as the root label. Do not switch the root to the
related/filter label just because that label owns the filtered property.

CRITICAL LIMITS
• NEVER include limit when select is present (sum/avg/min/max/count/collect/timeBucket).
  limit restricts the record scan → results are mathematically wrong.
  e.g. "total budget of all 33 projects" with limit:10 returns only the sum of the first 10.
• Self-group and dimensional groupBy queries: omit limit entirely (or scope to root records only).
• limit is valid only for listing/browsing queries or per-record flat aggregation (one row per root record).

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§1) WHERE — COMPLETE FILTER & TRAVERSAL REFERENCE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

ALL label, field, and relationship-type names in this document are placeholders — real
names come exclusively from getSchemaMarkdown/discovery, never from these examples.

The where clause mechanism: when a nested object key is NOT a criteria operator (like $gt,
$contains, etc.) and NOT a flat value, RushDB interprets that key as the LABEL of a related
record to traverse.

── PREDICATE VALUES MUST BE LITERALS ─────────────────────────────────
  where values are literals (string / number / boolean / date) only. A value can NEVER be a
  field or alias reference:
    fieldA: "$record.id"          // WRONG — matched as the literal text "$record.id", returns nothing
    fieldA: { $eq: "$alias.id" }  // WRONG — same problem inside an operator
    fieldA: { $ref: "$record.id" } // WRONG — $ref exists only in select expressions
  RushDB has no correlated where predicate (no "join on fieldA = fieldB") in ANY syntax.
  $record.*, $alias.*, and $relation references are valid ONLY in select / groupBy / aggregate.
  To rank or group by a scalar field that is NOT a relationship in the schema, root on the label
  that owns the field and groupBy that field — do not fabricate a related-label traversal or a
  correlated filter.

── PRIMITIVE VALUE MATCHING ──────────────────────────────────────────
Direct equality, all types:
  name: "John Doe"                       // exact string (case-sensitive equality)
  name: { $eq: "John Doe" }              // exact string alias, useful for MongoDB-style queries
  isActive: true                         // boolean
  isActive: { $eq: true }                // exact boolean alias
  age: 30                                // number
  age: { $eq: 30 }                       // exact number alias
  created: "2023-01-01T00:00:00Z"        // ISO 8601 datetime

── STRING OPERATORS ─────────────────────────────────────────────────
  name: { $contains: "John" }            // substring match (case-insensitive)
  name: { $startsWith: "J" }             // prefix match (case-insensitive)
  name: { $endsWith: "son" }             // suffix match (case-insensitive)
  name: { $eq: "John" }                  // exact equality alias (case-sensitive)
  name: { $ne: "deleted" }               // not equal
  status: { $in: ["active","pending"] }  // matches any value in array
  status: { $nin: ["deleted","archived"] } // matches none of these values

  For user-provided named references that may be incomplete, abbreviated, or shortened,
  prefer $contains on the label's display property resolved from schema discovery
  (getSchemaMarkdown/findProperties) — often name or title, but NEVER assume such a field
  exists; if discovery shows none, pick from the schema's string properties or ask.
  Use exact equality only for exact IDs, canonical full values, or explicit exact-match requests.

── NUMBER OPERATORS ──────────────────────────────────────────────────
  age: { $gt: 18 }        // greater than
  age: { $gte: 21 }       // greater than or equal
  age: { $lt: 65 }        // less than
  age: { $lte: 64 }       // less than or equal
  age: { $eq: 18 }        // exact equality alias
  age: { $ne: 18 }        // not equal
  age: { $in: [20,30,40] }   // matches any of these numbers
  age: { $nin: [20,30,40] }  // matches none of these numbers

── BOOLEAN OPERATORS ────────────────────────────────────────────────
  isActive: true                  // direct match
  isActive: { $eq: true }         // exact equality alias
  isActive: { $ne: false }        // not equal (matches true or unset)

── DATETIME OPERATORS ───────────────────────────────────────────────
  ISO 8601 strings are valid for equality, $in, and range operators ($gt/$gte/$lt/$lte).
  Use component objects for calendar-semantic boundaries (year / month / day) where
  expressing the range as calendar units is clearer than computing a UTC timestamp.

  // ISO 8601 — valid for equality, $in, and range operators:
  created: "2023-01-01T00:00:00Z"
  created: { $in: ["2023-01-01T00:00:00Z", "2023-02-01T00:00:00Z"] }
  created: { $gte: "2023-01-01T00:00:00Z" }
  Relative ("last 7 days", "this month"): compute ISO UTC boundary → use ISO string with $gte/$lte.

  // Component matching (exact point in time):
  created: { $year: 2023, $month: 1, $day: 1 }
  // Available components: $year $month $day $hour $minute $second $millisecond $microsecond $nanosecond

  // Calendar-semantic ranges — component objects are clearest:
  Year   "in 1994":    { field: { $gte: { $year:1994 }, $lt: { $year:1995 } } }
  Month  "Jan 1994":   { field: { $gte: { $year:1994,$month:1 }, $lt: { $year:1994,$month:2 } } }
  Day    "1994-03-15": { field: { $gte: { $year:1994,$month:3,$day:15 }, $lt: { $year:1994,$month:3,$day:16 } } }
  Decade "1990s":      { field: { $gte: { $year:1990 }, $lt: { $year:2000 } } }

  // Month+day WITHOUT year: unsupported — ask the user for a year. Do not mention internal reasons.

── FIELD EXISTENCE & TYPE ───────────────────────────────────────────
  phoneNumber: { $exists: true }    // only records that have this field (not null/empty)
  phoneNumber: { $exists: false }   // only records that do NOT have this field
  age: { $type: "number" }          // "string"|"number"|"boolean"|"datetime"

── LOGICAL GROUPING OPERATORS ───────────────────────────────────────
  // Implicit $and (multiple keys at same level = AND):
  where: { name: { $startsWith: "J" }, age: { $gte: 21 } }

  // Explicit versions:
  $and: [ { name: { $startsWith: "J" } }, { age: { $gte: 21 } } ]
  $or:  [ { name: { $startsWith: "J" } }, { age: { $gte: 21 } } ]
  $not: { status: "deleted" }
  $nor: [ { status: "deleted" }, { status: "archived" } ]
  $xor: [ { isPremium: true }, { hasFreeTrialAccess: true } ]   // exactly one must match

  // Nested logical grouping:
  $or: [
    { status: "active" },
    { $and: [ { status: "pending" }, { createdAt: { $gte: "2023-01-01T00:00:00Z" } } ] }
  ]

── RELATIONSHIP TRAVERSAL ────────────────────────────────────────────
Traversal rule: ANY nested key whose value is an object that is not operator criteria is
interpreted as a related-record traversal, not a field filter. The key IS a label copied
exactly as spelled in the schema — labels are case-sensitive and may be any case
(UPPER_CASE is a common naming convention, not a requirement). Never change a label's case.
A traversal block REQUIRES the related record to exist: the compiler adds a
"recordN IS NOT NULL" check. OPTIONAL MATCH is used internally only so $or/$not/$nor
between sibling blocks composes — to express absence, wrap the block in $not.

Basic (filter by related record properties):
  where: {
    name: "Tech Corp",
    DEPARTMENT: {                        // traverse to related DEPARTMENT records
      name: "Engineering",
      headcount: { $gte: 10 }
    }
  }

Multi-level nesting (path):
  where: {
    DEPARTMENT: {
      name: "Engineering",
      PROJECT: {                         // DEPARTMENT → PROJECT
        name: "Database",
        EMPLOYEE: { role: "Developer" }  // PROJECT → EMPLOYEE
      }
    }
  }

⚠ TRAVERSAL SYNTAX — COMMON HALLUCINATION (ALWAYS WRONG):
  The operators $label / $direction / $as / $of / $through / $hops DO NOT EXIST.
  Never write: { "employee": { "$label": "EMPLOYEE", "$as": "$emp", "$direction": "out" } }
  The key IS the label. There is no $label operand. Alias is always $alias, not $as.
  Multihop depth lives INSIDE $relation as hops, never as a standalone $hops operator.
  WRONG:   where: { employee: { $label:'EMPLOYEE', $direction:'out', $as:'$emp' } }
  CORRECT: where: { EMPLOYEE: { $alias:'$emp' } }   // key = label; alias via $alias only
  ($relation.hops and $cycle ARE valid — see below.)

$alias — name a traversed node for use in select/groupBy:
  where: {
    DEPARTMENT: {
      $alias: '$department',
      PROJECT: {
        $alias: '$project',
        EMPLOYEE: { $alias: '$employee' }
      }
    }
  }

$relation — constrain relationship type and/or direction:
  where: {
    POST: {
      $relation: { type: 'AUTHORED', direction: 'in' },  // full form
      title: { $contains: 'Graph' }
    }
  }
  Shorthand (type only): $relation: 'AUTHORED'
  direction options: 'in' | 'out'  (omit = any direction)
  type is OPTIONAL — omitting it traverses any relationship, which is valid (including with hops).

  ⚠ TYPE COMES FROM THE SCHEMA, VERBATIM: copy $relation.type exactly from a schema
  Relationships row — never invent a semantic type like the AUTHORED/REPORTS_TO examples here,
  and never write __RUSHDB__RELATION__DEFAULT__ unless a schema row shows that exact type.
  When it does (data imported as nested JSON often has only such edges), either use that
  exact string or omit type entirely.

  READING THE SCHEMA: each Relationships row is a directed pattern rooted at that label.
    (SELF)-[:TYPE]->(OTHER)   → from SELF, traverse OTHER with $relation { type:'TYPE', direction:'out' }
    (SELF)<-[:TYPE]-(OTHER)   → from SELF, traverse OTHER with $relation { type:'TYPE', direction:'in' }
  Only patterns listed in the schema are traversable. A scalar reference property (e.g. author_id)
  is a plain value, NOT an edge — do not nest a label to "join" on it; root on the label that owns it
  and groupBy that field instead.

$relation.hops — variable-length traversal (multihop over ONE pattern):
  where: {
    EMPLOYEE: {
      $alias: '$manager',
      $relation: { type: 'REPORTS_TO', direction: 'out', hops: { min: 1, max: 4 } },
      name: { $contains: 'Alice' }
    }
  }
  hops: 3 = exactly 3 hops; hops: { min?, max? } = range (min defaults to 1).
  Use for hierarchies ("any ancestor/descendant", "whole reporting chain"), "within N degrees",
  or transitive closure over one relationship type. The endpoint label constrains only the FINAL
  record; intermediates are unconstrained.
  • Keep max as small as the request allows; deep undirected traversal is expensive. Omit type
    only when the user genuinely means "any relationship"; keep direction when the schema gives one.
  • Always set hops.max (an integer as small as the request allows). Omitting max means
    unbounded traversal, which is rejected on shared cloud connections and only accepted on
    self-hosted / dedicated-database setups; hops.max is capped per deployment (default 10).
  • Endpoint aggregations apply per PATH: $count/$collect deduplicate, but $sum/$avg over a
    multihop alias count one row per path — prefer counting to summing across multihop endpoints.
  • One hop stays the default: never add hops for a plain related-record condition.

$cycle — cycle/ring detection (a record-level predicate; the path binds back to the record):
  where: {
    $cycle: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
  }
  Matches roots sitting on a closed path (fraud rings, circular ownership, dependency cycles).
  The operator's value IS the traversal spec (type, direction, hops — hops mandatory, min ≥ 2,
  defaults to 2). No $alias, no property criteria, no nested labels — a cycle has no separate
  endpoint. Intermediate node labels are unconstrained. Place inside a label block
  (ACCOUNT: { country: 'US', $cycle: {...} }) to anchor the cycle at the related record.
  Set direction for flow-like semantics (money, ownership); undirected cycles also match
  innocent back-and-forth pairs. Wrap in $not for "NOT on a cycle". Paths may revisit a record
  via different relationships (only relationships are unique per path).
  Never wrap $cycle in a named block and never nest $relation inside it — the value IS the
  relation spec, and it sits directly at a where level like any other operator.

$id — filter by record ID (supports $in, $nin, string operators):
  where: { $id: { $in: ['id1','id2'] } }
  where: { EMPLOYEE: { $id: 'specific-id' } }

── LOGICAL GROUPING WITH RELATIONSHIPS ──────────────────────────────
Logical operators can wrap entire relationship blocks:
  where: {
    $or: [
      { DEPARTMENT: { name: "Engineering" } },
      { DEPARTMENT: { name: "Product" } }
    ]
  }
  where: {
    $and: [
      { DEPARTMENT: { name: "Engineering" } },
      { PROJECT: { budget: { $gte: 10000 } } }
    ]
  }
  where: {
    name: "Tech Corp",
    $or: [
      { DEPARTMENT: { name: "Engineering" } },
      { DEPARTMENT: { name: "Product", $not: { PROJECT: { status: "Canceled" } } } }
    ]
  }

Logical operators INSIDE relationship blocks:
  where: {
    DEPARTMENT: {
      $or: [ { name: "Engineering" }, { name: "Product" } ],
      PROJECT: {
        $and: [ { budget: { $gte: 10000 } }, { status: { $ne: "Canceled" } } ]
      }
    }
  }

── KEY BEHAVIORAL NOTES ─────────────────────────────────────────────
  • Field names are case-sensitive.
  • Missing fields are NOT matched — { active: true } skips records without an 'active' field.
  • String operators ($contains, $startsWith, $endsWith) are case-insensitive.
  • Array fields: condition satisfied if ANY element matches (tags:"typescript" matches ["js","typescript"]).
  • A traversal block requires the related record to exist (the compiler adds a
    "recordN IS NOT NULL" check); use $not around the block to express absence.
  • Logical operators work at ANY nesting level, including inside relationship blocks.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§2) SELECT — EXPRESSION-BASED OUTPUT SHAPING
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

\`select\` is the canonical output-shaping clause. Every key maps to an Expr:

EXPRESSION TYPES:
  "$record.field"           — field reference (project a field value)
  42 / true                 — literal number or boolean
  { $ref: "otherKey" }      — reference another select output key (for derived metrics)
  { $sum: expr }            — sum of an expression
  { $avg: expr }            — average; add $precision:N to round
  { $count: "*" | expr }    — count DISTINCT records or field values
  { $min: expr }            — minimum value
  { $max: expr }            — maximum value
  { $divide: [A, B] }       — A / B
  { $multiply: [A, B] }     — A * B
  { $add: [A, B] }          — A + B
  { $subtract: [A, B] }     — A - B
  { $collect: CollectExpr } — gather related records into an array
  { $timeBucket: TBExpr }   — time-series bucketing

BASIC EXAMPLES:
  select: {
    name:     "$record.name",
    total:    { $sum: "$record.amount" },
    count:    { $count: "*" },
    avg:      { $avg: "$record.value" },
    rounded:  { $avg: "$record.value", $precision: 2 }
  }

DERIVED METRICS via $ref (evaluated AFTER all non-$ref expressions):
  select: {
    revenue: { $sum: "$record.amount" },
    cost:    { $sum: "$record.cost" },
    profit:  { $subtract: [{ $ref: "revenue" }, { $ref: "cost" }] },
    margin:  { $divide:   [{ $ref: "profit" },  { $ref: "revenue" }] }
  }

MATH inside aggregation:
  select: {
    total: { $sum: { $multiply: ["$record.price", "$record.quantity"] } }
  }

COLLECT from related node — two forms:

FORM A (alias-based — requires $alias in where):
  where:  { USER: { $alias: "$user" } },
  select: {
    users: {
      $collect: {
        from:    "$user",
        select:  { id: "$user.id", name: "$user.name" },
        orderBy: { name: "asc" },
        limit:   10
      }
    }
  }

FORM B (label-based — no $alias needed; $self = current level; supports unlimited nesting):
  select: {
    departments: {
      $collect: {
        label: "DEPARTMENT",
        where: { budget: { $gte: 10000 } },  // optional flat filter on this level
        select: {
          name: "$self.name",
          projects: {
            $collect: {
              label: "PROJECT",
              select: {
                name: "$self.name",
                employees: {
                  $collect: { label: "EMPLOYEE", orderBy: { salary: "desc" }, limit: 3 }
                }
              }
            }
          }
        }
      }
    }
  }

TIMEBUCKET:
  select: {
    bucket: { $timeBucket: { field: "$record.createdAt", unit: "day" } },
    count:  { $count: "*" }
  },
  groupBy: ["bucket"]

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§3) GROUPBY — TWO MODES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

groupBy is valid only with select.

Do not use alias-only values in groupBy. "$record" and "$related" are invalid because
grouped rows need a concrete dimension field. Use "$record.name", "$related.status",
or a select output key such as "totalBudget".

MODE A — DIMENSIONAL (one row per distinct value — "$alias.propertyName" format):
  select: { count: { $count: "*" }, avg: { $avg: "$record.total", $precision: 2 } },
  groupBy: ['$record.status'],
  orderBy: { count: 'desc' }
  Output rows: [{ "status":"pending","count":120,"avg":310.42 }, ...]
  Note: group key appears WITHOUT alias prefix in output  ($record.status → "status").
  Multiple keys = pivot: groupBy: ['$record.category','$record.active'] → one row per (category,active) pair.

MODE B — SELF-GROUP (collapse everything to ONE row with global metric(s)):
  Put the SELECT KEY NAMES themselves in groupBy (not property paths):
  select: { totalBudget: { $sum: "$record.budget" } },
  groupBy: ['totalBudget']
  Output: [{ "totalBudget": 1875251446 }]
  Multiple KPIs: groupBy: ['totalRevenue','orderCount'] → [{ "totalRevenue":987654, "orderCount":420 }]

⚠ LATE-ORDERING RULE — CRITICAL FOR CORRECT TOTALS:
  When orderBy references a select key, the engine applies ORDER BY + LIMIT
  AFTER the aggregation (full-scan first, then paginate).
  When orderBy is absent or references a raw field, LIMIT is applied BEFORE aggregation
  → only the first N raw records are aggregated → WRONG totals.
  FIX: for self-group and any pure metric query, always add orderBy on the select key:
    select:{ total:{ $sum: "$record.amount" } },
    groupBy:['total'],
    orderBy:{ total:'asc' }   ← triggers late ordering; ensures full dataset is summed

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§4) COLLECT — ARRAY GATHERING & NESTED STRUCTURES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Use select $collect (label-based) for all nested queries:

  labels: ['COMPANY'],
  select: {
    departments: {
      $collect: {
        label: 'DEPARTMENT',
        select: {
          name: '$self.name',         // $self = current traversal level
          projects: {
            $collect: {
              label: 'PROJECT',
              select: {
                name: '$self.name',
                employees: { $collect: { label: 'EMPLOYEE', orderBy: { salary: 'desc' }, limit: 3 } }
              }
            }
          }
        }
      }
    }
  }
  → returns COMPANY → [DEPARTMENT → [PROJECT → [top-3 EMPLOYEE]]] tree with no $alias boilerplate.
  → use where: { field: condition } inside any $collect level to filter that traversal level.

Collect options (inside $collect):
  label    — related label to collect from (label-based form)
  from     — $alias string (alias-based form; requires $alias in where)
  where?   — flat filter on this traversal level
  select?  — shape the collected records (omit to collect entire records)
  orderBy? — sort collected items: { salary: 'desc' }
  limit?   — max items in the array
  skip?    — skip N items in the collected array

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§5) LIMIT RULES BY QUERY MODE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  • Self-group (single KPI row): NO limit — but MUST add orderBy on the select key for late ordering.
  • Dimensional groupBy: NO limit to get all groups; add limit + orderBy on select key for "top N".
  • Per-record flat aggregation (one row per root record): limit IS valid (caps root records).
  • Pure listing (no select): limit is always valid.
  • "how many" simple count: read 'total' from the findRecords response — do NOT use $count for this.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§6) METRIC FIELD DISCOVERY ACROSS RELATED LABELS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
If the metric field is NOT on the target label, search related labels before giving up:
  1) Confirm target label. findProperties(labels:[<target>]) — look for the metric field.
  2) If absent, walk adjacent labels via getSchemaMarkdown or findRelationships with source/target endpoint probes.
  3) For each candidate related label R: findProperties(labels:[R]) and attempt the same match.
  4) When found on CHILD: where:{ CHILD:{ ...filters..., $alias:'$child' } }, select referencing '$child.*'.
     Root-level filters (status, dates) stay at the top-level where.
  5) Never abandon after one miss — always attempt at least one related-label discovery pass.

Related-count ranking:
  For "which/what <parent> has most/more/least/less/fewer/fewest <related records>",
  root the parent label, traverse the related label in where with $alias, put related
  filters inside that related-label block, count the related alias, group by the parent's
  display property (from the schema), and order the count.
  Direction: most/more/highest/largest/greatest → desc; least/less/fewer/fewest/lowest/smallest → asc.
  If the parent→related traversal path is absent, do not silently root on the related label;
  ask or return the closest valid query with an explicit assumption.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§7) RANGE / DISTRIBUTION QUERIES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  • type = number or datetime → findRecords with select:{ min:{ $min:'$record.<field>' }, max:{ $max:'$record.<field>' } }
    plus groupBy:['min','max']. Or: getSchema (JSON) → propertyValues(propertyId) → returns { min, max } directly.
  • type = string or boolean → propertyValues(propertyId) to list all distinct values.
  • NEVER call findRecords with a where filter to "search for" values of a field — that returns records, not ranges.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§8) MULTI-LABEL FILTER DISTRIBUTION
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Place each filter with the label that actually holds the field. On zero results, silently retry
by moving the filter to the related child block before asking.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§9) ENUM / VALUE NORMALIZATION
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Never hardcode guessed values for enumerated fields:
  1) findProperties to locate the property and get its id.
  2) propertyValues(propertyId, { query:<user value> }) to probe existing values.
  3) If no match: try case variants, abbreviations, partial prefixes.
  4) Use ONLY canonical values returned by propertyValues.
  5) Re-run propertyValues with empty query to list top candidates; retry once silently.
     Ask only if two+ equally plausible values remain.
  6) Always mention assumption briefly if mapping is non-obvious, then proceed.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§10) RELATIONSHIP & PATH QUERIES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Entity resolution by name: probe with findRecords(limit:1, where:{ <nameField>:{ $contains:'...' } }).

MULTI-HOP RELATIONSHIP DISCOVERY
Two native tools, pick by shape:
  a) SAME pattern repeated N times (self-referencing hierarchy, "within N degrees",
     transitive closure over one relationship type) → $relation.hops on a single block:
     where:{ EMPLOYEE:{ $alias:'$report', $relation:{ type:'REPORTS_TO', direction:'in', hops:{ max:5 } } } }
     No intermediate wrappers, no BFS needed.
  b) DIFFERENT labels chained (PARENT→A→B→CHILD) → nested label blocks, one per hop.
     Discover the chain first (steps below); hops does NOT help here because each hop
     is a different pattern.

Pre-check before multi-hop:
  1) findProperties(labels:[parent]) for direct scalar fields.
  2) Fetch 1 sample parent record. findRelationships with source.where.$id or target.where.$id → discover adjacent labels.
  3) If direct path to child exists: where:{ CHILD:{ $alias:'$child' } } — STOP. No intermediate wrappers.
  4) If the schema shows a self-referencing pattern on one label: use $relation.hops (shape a).
  5) Only if neither: BFS over distinct labels (depth ≤ 4).

BFS algorithm (shape b):
  1) Resolve parent + child labels via findLabels.
  2) Fetch 1 sample record of current hop; findRelationships with source.where.$id or target.where.$id → adjacent labels.
  3) On finding path PARENT→A→B→CHILD:
     where:{ A:{ B:{ CHILD:{ $alias:'$child' } } } }
     select:{ metric:{ $count:'*' } }
  4) No top-level limit for pure grouped aggregations.
  5) Only the root parent label appears in labels:[]. Intermediates appear only inside where.
  6) NEVER reuse '$record' alias for related-node references.
  7) After path found, collapse redundant intermediate layers.
  8) If BFS exhausts without match: synonym remap using findProperties output; if still unavailable, ask.

Avoid over-nesting:
  WRONG:   where:{ A:{ B:{ $alias:'$b' } } }   ← when B is directly linked to root
  CORRECT: where:{ B:{ $alias:'$b' } }
  WRONG:   where:{ EMPLOYEE:{ EMPLOYEE:{ EMPLOYEE:{} } } }   ← same label chained manually
  CORRECT: where:{ EMPLOYEE:{ $relation:{ type:'REPORTS_TO', direction:'out', hops:{ max:3 } } } }

CYCLE / RING DETECTION
"Records on a loop back to themselves" (fraud rings, circular ownership, dependency cycles)
→ $cycle operator (see §1). Root label = the entity type; the operator's value is the traversal spec:
  findRecords({ labels:['ACCOUNT'],
    where:{ $cycle:{ type:'TRANSFERRED_TO', direction:'out', hops:{ min:2, max:6 } } } })
Returns ring PARTICIPANTS. To reconstruct a specific ring's composition, follow up with
bounded one-hop queries from each flagged record.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§11) NL → WHERE TRANSLATION QUICK REFERENCE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

• Numerics: 1k=1000, 1m=1000000, 1b=1000000000. Strip currency symbols ($100k→100000).
• Equality / sets:
    field: value
    field: { $eq: value }     // exact equality alias for MongoDB-style syntax
    field: { $ne: value }
    field: { $in: [v1,v2] }
    field: { $nin: [v1,v2] }
• Numbers:  > $gt   >= $gte   < $lt   <= $lte   between X and Y → { $gte:X, $lte:Y }
• Strings (case-insensitive): $contains / $startsWith / $endsWith / $in / $nin / $ne.
• Booleans: field: true|false or { field: { $ne: value } }.
• Datetime — always component objects for ranges; see §1 datetime operators above.
• Logical: $and / $or / $not / $nor / $xor. Prefer implicit AND when simple.
• "any ancestor/descendant", "reporting chain", "within N hops/degrees/steps"
    → $relation: { type?, direction, hops:{ max:N } } on ONE traversal block
      (type verbatim from the schema, or omitted).
• "ring", "loop", "circular …", "cycles back to itself"
    → { $cycle: { type?, direction, hops:{ min:2, max:N } } }.
• Field names are case-sensitive. String comparisons are case-insensitive by default.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§12) VALIDATION CHECKLIST
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Before submitting a findRecords call, verify:
□ No groupBy without select.
□ limit absent for self-group and dimensional groupBy (unless scoping root records in flat select).
□ orderBy on select key present for self-group queries (triggers late ordering → correct totals).
□ groupBy mode correct:
    Dimensional: entries are "$alias.propertyName" strings.
    Self-group: entries are select key names (no dot, no alias prefix).
□ No alias-only groupBy values such as "$record" or "$related".
□ Root labels only in labels; related labels go in where traversal with $alias if referenced.
□ Related-count ranking keeps the requested parent/entity as root; the related/filter label does not steal the root.
□ Traversal: key = a label exactly as spelled in the schema (case-sensitive; do not
    change its case). NEVER $label/$direction/$as/$of/$through/$hops.
    WRONG: { employee: { $label:'EMPLOYEE' } }   CORRECT: { EMPLOYEE: { $alias:'$emp' } }
    ($relation.hops and $cycle are VALID operators — do not "correct" them away.)
□ $relation.type copied verbatim from a schema Relationships row, or omitted; never invented.
□ hops only for repeated-pattern traversal (hierarchy/degrees); bounded max, as small as possible.
□ $cycle value is the traversal spec itself (hops mandatory, min ≥ 2); never a named block, never $relation nested inside.
□ Vector threshold semantics: euclidean → $lte; others → $gte.
□ Month+day without year → ask for year.
□ Aggregation intent? → query MUST include select + groupBy. Raw records ≠ aggregation.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
§13) EXAMPLE PATTERNS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
(Actual label/field names always come from getSchemaMarkdown — never from these examples.)

List with numeric filter:
  findRecords({ labels:['<LABEL>'], where:{ <field>:{ $gt:100000 } }, limit:10 })

Date range:
  findRecords({ labels:['<LABEL>'], where:{ <dateField>:{ $gte:{ $year:1994 }, $lt:{ $year:1995 } } }, limit:10 })

Dimensional groupBy (count + avg per category):
  findRecords({
    labels:['<LABEL>'],
    select:{ count:{ $count:'*' }, avg:{ $avg:'$record.<field>', $precision:2 } },
    groupBy:['$record.<categoryField>'],
    orderBy:{ count:'desc' }
  })

Which parent has the most/fewest related children:
  findRecords({
    labels:['<PARENT_LABEL>'],
    where:{ <CHILD_LABEL>:{ $alias:'$child' } },
    select:{
      parent:'$record.<nameField>',
      children:{ $count:'$child' }
    },
    groupBy:['$record.<nameField>'],
    orderBy:{ children:'desc' }
  })
  → use desc for most/more/highest/largest/greatest, asc for least/less/fewer/fewest/lowest/smallest.

Self-group single KPI:
  findRecords({
    labels:['<LABEL>'],
    select:{ total:{ $sum:'$record.<field>' } },
    groupBy:['total'],
    orderBy:{ total:'asc' }    ← required for correct full-scan total
  })

Self-group multiple KPIs:
  findRecords({
    labels:['<LABEL>'],
    select:{
      totalRevenue:{ $sum:'$record.<revenueField>' },
      orderCount:  { $count:'*' },
      avgOrder:    { $avg:'$record.<revenueField>', $precision:2 }
    },
    groupBy:['totalRevenue','orderCount','avgOrder'],
    orderBy:{ totalRevenue:'asc' }
  })

Per-record from related label (one row per root record):
  labels:['PROJECT'],
  where:{ budget:{ $lte:10000000 }, EMPLOYEE:{ $alias:'$employee' } },
  select:{
    projectName:  '$record.name',
    headcount:    { $count:'$employee.id' },
    totalWage:    { $sum:'$employee.salary' },
    avgSalary:    { $avg:'$employee.salary', $precision:0 }
  },
  limit:100   ← valid: caps root PROJECT records, not the scan for aggregation

Nested hierarchy (COMPANY → DEPT → PROJECT → EMPLOYEE):
  labels:['COMPANY'],
  where:{ foundedAt:{ $lte:{ $year:1980 } } },
  select:{
    company: '$record.name',
    departments:{
      $collect:{
        label:'DEPARTMENT',
        select:{
          name:'$self.name',
          projects:{
            $collect:{
              label:'PROJECT',
              orderBy:{ name:'asc' },
              select:{
                name:'$self.name',
                employees:{ $collect:{ label:'EMPLOYEE', orderBy:{ salary:'desc' }, limit:3 } }
              }
            }
          }
        }
      }
    }
  }

Time series monthly count:
  findRecords({
    labels:['<LABEL>'],
    select:{ month:{ $timeBucket:{ field:'$record.<dateField>', unit:'month' } }, count:{ $count:'*' } },
    groupBy:['month'], orderBy:{ month:'asc' }
  })

Relationship with specific type/direction:
  where:{ POST:{ $relation:{ type:'AUTHORED', direction:'in' }, title:{ $contains:'Graph' } } }

Multihop hierarchy ("everyone in Alice's reporting chain, up to 4 levels"):
  where:{ EMPLOYEE:{ $relation:{ type:'REPORTS_TO', direction:'out', hops:{ max:4 } }, name:{ $contains:'Alice' } } }

Cycle detection ("accounts on a circular transfer ring"):
  findRecords({ labels:['ACCOUNT'],
    where:{ $cycle:{ type:'TRANSFERRED_TO', direction:'out', hops:{ min:2, max:6 } } } })

Filter by ID:
  where:{ $id:{ $in:['id1','id2'] } }
  where:{ EMPLOYEE:{ $id:'specific-id' } }

XOR / exclusive range:
  where:{ budget:{ $xor:{ $lte:10000000, $gte:15000000 } } }

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Resource-local where rule:
  - findRecords.where, exportRecords.where, and bulkDeleteRecords.where apply to Records.
  - findRelationships.where applies to relationship edges: type maps to type(rel), all
    other fields map to user-defined relationship properties. Use source/target for
    endpoint Record predicates.
  - findLabels.where and findProperties.where apply to Records before returning
    matching label/property metadata.
findRelationships does not support select/groupBy.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
` as const

export default SEARCH_QUERY_SPEC
