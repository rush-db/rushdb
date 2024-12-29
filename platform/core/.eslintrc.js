module.exports = {
    parser: '@typescript-eslint/parser',
    parserOptions: {
        project: 'tsconfig.json',
        sourceType: 'module',
    },
    plugins: ['@typescript-eslint/eslint-plugin', 'import'],
    extends: [
        'plugin:@typescript-eslint/recommended',
        'eslint:recommended',
        'plugin:@typescript-eslint/eslint-recommended',
        'plugin:@typescript-eslint/recommended',
    ],
    root: true,
    env: {
        node: true,
        jest: true,
    },
    ignorePatterns: ['.eslintrc.js'],
    rules: {
        curly: ['error'],
        'no-useless-escape': 'off',
        '@typescript-eslint/naming-convention': [
            'warn',
            {
                selector: 'typeAlias',
                format: ['PascalCase'],
            },
            {
                selector: 'interface',
                format: ['PascalCase'],
            },
        ],
        'no-prototype-builtins': 'off',
        '@typescript-eslint/interface-name-prefix': 'off',
        '@typescript-eslint/no-empty-interface': 'off',
        '@typescript-eslint/no-unused-vars': 'off',
        '@typescript-eslint/explicit-function-return-type': 'off',
        '@typescript-eslint/explicit-module-boundary-types': 'off',
        '@typescript-eslint/no-explicit-any': 'off',
        '@typescript-eslint/no-use-before-define': 'off',
        '@typescript-eslint/ban-types': 'warn',
        'import/order': [
            'error',
            {
                groups: [
                    'external',
                    'internal',
                    'builtin',
                    'parent',
                    'sibling',
                    'index',
                    'object',
                    'type',
                ],
                alphabetize: { order: 'asc', caseInsensitive: true },
                pathGroups: [
                    {
                        pattern: '@/**',
                        group: 'internal',
                        position: 'before',
                    },
                    {
                        pattern: 'src/**',
                        group: 'internal',
                    },
                ],
                'newlines-between': 'always',
                pathGroupsExcludedImportTypes: ['type'],
                warnOnUnassignedImports: true,
            },
        ],
    },
};
