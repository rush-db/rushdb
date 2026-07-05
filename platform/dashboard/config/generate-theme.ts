/**
 * Emits `src/theme.css` from the culori-derived palette in `config/colors.ts`:
 * the Tailwind v4 `@theme inline` color tokens plus the runtime dark/light
 * CSS-variable sets that power theme switching (`[data-theme='light']`).
 *
 * Run with `pnpm theme:generate` (wired into `dev` and `build`) after any
 * palette change.
 */
import { writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { darkVars, lightVars, themeTokens } from './colors'

const outFile = resolve(dirname(fileURLToPath(import.meta.url)), '../src/theme.css')

const declarations = (vars: Record<string, string>, indent: string) =>
  Object.entries(vars)
    .map(([name, value]) => `${indent}${name}: ${value};`)
    .join('\n')

const css = `/* AUTO-GENERATED — do not edit by hand.
 * Source: config/colors.ts. Regenerate with \`pnpm theme:generate\`.
 */

@theme inline {
${declarations(themeTokens, '  ')}
}

@layer base {
  :root {
    color-scheme: dark;
${declarations(darkVars, '    ')}
  }

  [data-theme='light'] {
    color-scheme: light;
${declarations(lightVars, '    ')}
  }
}
`

writeFileSync(outFile, css)
console.log(`Wrote ${Object.keys(themeTokens).length} theme tokens to ${outFile}`)
