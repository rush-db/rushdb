/**
 * Root-level end-to-end suite: boots the full platform (Neo4j + Postgres via docker,
 * platform/core via node) and exercises it black-box over HTTP with the real JS SDK.
 *
 * Run from the repo root with `pnpm test:e2e`.
 * Set E2E_BASE_URL to target an already-running stack and skip provisioning.
 */
module.exports = {
  rootDir: '.',
  testEnvironment: 'node',
  // Platform specs use *.e2e-spec.ts; the SDK suites (e2e/sdk/) use *.e2e.test.ts.
  testRegex: '\\.e2e(-spec|\\.test)\\.ts$',
  resolver: 'jest-ts-webcompat-resolver',
  transform: {
    '^.+\\.(t|j)sx?$': '@swc/jest'
  },
  globalSetup: '<rootDir>/setup/global-setup.ts',
  globalTeardown: '<rootDir>/setup/global-teardown.ts',
  setupFilesAfterEnv: ['<rootDir>/setup/sdk-env.ts'],
  maxWorkers: 1,
  testTimeout: 180_000
}
