E2E tests for @rushdb/javascript-sdk

Setup
- Create a .env file in this package (packages/javascript-sdk/.env) with:

  RUSHDB_API_KEY=your_api_key_here

Run
- From repo root:

  pnpm -w dlx jest packages/javascript-sdk/tests/relationships.createMany.e2e.test.ts

Notes
- The test uses unique tenantId per run and will create sample USER and ORDER records.
- It then calls relationships.createMany and verifies via relationships.find with retry.
