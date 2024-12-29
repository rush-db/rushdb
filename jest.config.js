/** @type {import('ts-jest').JestConfigWithTsJest} */
module.exports = {
  resolver: "jest-ts-webcompat-resolver",
  collectCoverageFrom: ['tests/**/*.{ts,tsx,js,jsx}'],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'json', 'jsx'],
  roots: ['<rootDir>'],
  setupFilesAfterEnv: ['./setup-tests.ts'],
  transform: {
    '^.+\\.(t|j)sx?$': '@swc/jest'
  },
  transformIgnorePatterns: ['[/\\\\]node_modules[/\\\\].+\\.(t|j)sx?$']
}
