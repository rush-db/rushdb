# Contributing

---
We need to keep every import with preserved `.js` extension.

Explicitly including the `.js` extension in ESM imports ensures that modules are correctly resolved in Node.js and browser
environments, complies with the ECMAScript module specification, and maintains consistency and compatibility across
different tools and platforms.

Here is regexp to match broken imports:
```bash
import\s+[^'"\n]*\s*['"][^'"\n]*(?<!\.js)['"]
```


### Adding new changesets

To generate a new changeset, run `pnpm changeset` in the root of the repository. The generated markdown files in the .changeset directory should be committed to the repository.

### Releasing changes

1. Run `pnpm run version`. This will bump the versions of the packages previously specified with pnpm changeset (and any dependents of those) and update the changelog files.
2. Run `pnpm i`. This will update the lockfile and rebuild packages.
   Commit the changes.
3. Run `pnpm run publish`. This command will publish all packages that have bumped versions not yet present in the registry.

