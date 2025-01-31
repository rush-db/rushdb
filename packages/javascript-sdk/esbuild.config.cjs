const esbuild = require('esbuild')

// CommonJS build for Node.js and older environments
esbuild
  .build({
    entryPoints: ['src/index.node.ts'], // Entry point of your package
    outfile: 'dist/index.cjs', // Output file for CommonJS
    bundle: true, // Bundle all dependencies
    platform: 'node', // Target Node.js environment
    format: 'cjs', // Output format: CommonJS
    target: ['node12'], // Target Node.js version 12+
    tsconfig: 'tsconfig.json', // Use your TypeScript config
    minify: true // Minify the output
  })
  .catch(() => process.exit(1))

// ES Module build for modern environments
esbuild
  .build({
    entryPoints: ['src/index.worker.ts'], // Entry point of your package
    outfile: 'dist/index.mjs', // Output file for ES modules
    bundle: true, // Bundle all dependencies
    platform: 'neutral', // Platform-agnostic (works in browsers and Node.js)
    format: 'esm', // Output format: ES modules
    target: ['es2015'], // Target ES2015+ environments
    tsconfig: 'tsconfig.json', // Use your TypeScript config
    minify: true, // Minify the output
    external: ['http', 'https'] // Mark Node.js built-in modules as external
  })
  .catch(() => process.exit(1))
