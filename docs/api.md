# Programming API

Both jscpd v4 (TypeScript) and v5 (Rust) provide programmatic APIs for integration into your own tools.

## TypeScript (v4)

### `jscpd` Function

The `jscpd` function accepts an `argv`-style array and returns a `Promise<IClone[]>`:

```typescript
import { IClone } from '@jscpd/core';
import { jscpd } from 'jscpd';

const clones: IClone[] = await jscpd([]);
```

Pass options as CLI-like arguments:

```typescript
const clones: IClone[] = await jscpd([
  '', '', __dirname + '/../fixtures',
  '-m', 'weak',
  '--silent',
]);
```

### `detectClones` Function

A higher-level API with an options object:

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

### Custom Store

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

### Building Custom Tools

Compose the lower-level packages for deep customization:

- **`@jscpd/core`** — Core detection algorithm (Rabin-Karp), event emitter interface. Single dependency on `eventemitter3`.
- **`@jscpd/tokenizer`** — Source code tokenization (224+ formats via reprism).
- **`@jscpd/finder`** — File walking, clone detection orchestration, built-in reporters, subscribers, validators.
- **`@jscpd/leveldb-store`** — LevelDB persistent store for large repositories.
- **`@jscpd/redis-store`** — Redis store for distributed/CI environments.

See [Packages](./packages.md) for details on each package.

## Rust (v5)

The Rust engine is available as two npm packages — `jscpd@5` (installs both `jscpd` and `cpd` commands) and `cpd` (installs only `cpd`). On crates.io it is published as `jscpd`.

For integration in Rust applications, use the `cpd-finder` crate:

```rust
use cpd_finder::orchestrate::{RunConfig, run};

let config = RunConfig {
    paths: vec!["./src".into()],
    min_tokens: 50,
    ..Default::default()
};

let result = run(&config).unwrap();
println!("Found {} clones", result.clones.len());
println!("Analyzed {} files", result.statistics.total.sources);
```

### Crate Architecture

| Crate | Description |
|-------|-------------|
| `cpd-core` | Core data models and hashing (Rabin-Karp rolling hash) |
| `cpd-tokenizer` | Source code tokenization (223+ formats, uses `oxc_parser`) |
| `cpd-finder` | File walking, orchestration, git blame (`rayon` + `ignore` + `globset`) |
| `cpd-reporter` | Output format rendering (13 reporters) |

There is no Node.js API for v5 — use v4's TypeScript API for Node.js integration, or v5's Rust API for Rust integration.