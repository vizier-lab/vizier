import eslint from '@eslint/js'
import tseslint from 'typescript-eslint'

export default tseslint.config(
    {
        ignores: [
            'node_modules/**',
            'dist/**',
            'build/**',
            'coverage/**',
            '**/*.min.js',
        ],
    },
    eslint.configs.recommended,
    tseslint.configs.recommended,
    {
        rules: {
            // A name that starts with an underscore is one the code means to discard: the `node`
            // of a markdown element, a key taken out of a record. The rule cannot tell that from
            // a forgotten name, so the underscore is the signal.
            '@typescript-eslint/no-unused-vars': ['error', { varsIgnorePattern: '^_', argsIgnorePattern: '^_' }],
        },
    }
)
