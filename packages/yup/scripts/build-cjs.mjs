import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const cjs = `"use strict";
var __createBinding = (o, m, k, k2) => { if (k2 === undefined) k2 = k; Object.defineProperty(o, k2, { enumerable: true, get: function() { return m[k]; } }); };
var __setModuleDefault = (o, v) => { Object.defineProperty(o, "default", { enumerable: true, value: v }); };
var __toCommonJS = (mod) => { var result = {}; for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k); __setModuleDefault(result, mod); return result; };
var exportsObj = {};
(async () => {
  const m = await import('./index.js');
  module.exports = __toCommonJS(m);
})();
`
writeFileSync(resolve('dist', 'index.cjs'), cjs)
