# Programming API

jscpd v4 exposes a Node.js / TypeScript API so you can run detection from your own tools, build custom reporters, or reuse the tokenizer and detection core directly.

> The v5 engine on [`master`](https://github.com/kucherenko/jscpd) has no Node.js API. If you need `jscpd()` / `detectClones()` or the LevelDB/Redis stores, stay on `jscpd@4`.

Runnable versions of the snippets below live in [`examples/api`](../examples/api).

## `jscpd` Function

The `jscpd` function accepts an `argv`-style array and returns a `Promise<IClone[]>`:

```typescript
import { IClone } from '@jscpd/core';
import { jscpd } from 'jscpd';

const clones: IClone[] = await jscpd([]);
```

Pass options as CLI-like arguments (the first two entries stand in for `node` and the script path, as in `process.argv`):

```typescript
const clones: IClone[] = await jscpd([
  '', '', __dirname + '/../fixtures',
  '-m', 'weak',
  '--silent',
]);
```

## `detectClones` Function

A higher-level API with an options object (`IOptions` from `@jscpd/core`; the keys match the `.jscpd.json` config file):

```typescript
import { detectClones } from 'jscpd';

const clones = await detectClones({
  path: ['./src'],
  silent: true,
  format: ['javascript', 'typescript'],
  minLines: 5,
  minTokens: 50,
  mode: 'mild',
});
```

Reporters configured through `reporters` (for example `['json', 'html']`) run as part of the call and write to `output`.

## Custom Store

Use `detectClones` with a custom store for incremental detection:

```typescript
import { detectClones } from 'jscpd';
import { IMapFrame, MemoryStore } from '@jscpd/core';

const store = new MemoryStore<IMapFrame>();

await detectClones({
  path: ['./src'],
}, store);

// Re-use the store for incremental detection
await detectClones({
  path: ['./src'],
  silent: true,
}, store);
```

For large repositories, use the LevelDB store:

```typescript
import { detectClones } from 'jscpd';
import { IMapFrame } from '@jscpd/core';
import { LevelDBStore } from '@jscpd/leveldb-store';

const store = new LevelDBStore<IMapFrame>('/path/to/leveldb/dir');

await detectClones({
  path: ['./src'],
}, store);
```

## Building Custom Tools

Compose the lower-level packages for deep customization:

- **`@jscpd/core`** — Core detection algorithm (Rabin-Karp), event emitter interface, `IClone`, `IMapFrame`, `IOptions`, `IStatistic`, `MemoryStore`. Single dependency on `eventemitter3`.
- **`@jscpd/tokenizer`** — Source code tokenization (224 formats via reprism).
- **`@jscpd/finder`** — File walking (`getFilesToDetect`), `InFilesDetector`, built-in reporters, subscribers, validators, hooks.
- **`@jscpd/leveldb-store`** — LevelDB persistent store for large repositories.
- **`@jscpd/redis-store`** — Redis store for distributed/CI environments.

### Custom reporter

A reporter is any object with a `report(clones, statistic)` method:

```typescript
import { IClone, IStatistic } from '@jscpd/core';
import { InFilesDetector } from '@jscpd/finder';

const reporter = {
  report(clones: IClone[], statistic: IStatistic): void {
    console.log(`${clones.length} clones, ${statistic.total.percentage}% duplicated`);
  },
};

// detector: InFilesDetector
detector.registerReporter(reporter);
```

Third-party reporters published to npm (e.g. `jscpd-full-reporter`) are loaded by name from `--reporters` / `reporters` when they are installed alongside jscpd.

See [Packages](./packages.md) for details on each package.
