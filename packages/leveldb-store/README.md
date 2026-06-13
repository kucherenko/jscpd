# `@jscpd/leveldb-store`

> store for jscpd, used for large repositories

## Installation

```
npm install @jscpd/leveldb-store --save
```

## Usage

```typescript
import {Tokenizer} from '@jscpd/tokenizer';
import {
    Detector,
    IOptions,
    IClone,
    IStore,
    ITokenizer
} from '@jscpd/core';
import LeveldbStore from '@jscpd/leveldb-store';

const options: IOptions = {
    minLines: 5,
    maxLines: 500,
}

const tokenizer: ITokenizer = new Tokenizer();

// Default: stores cache in .jscpd/ relative to cwd
const store: IStore = new LeveldbStore();

const detector = new Detector(tokenizer, store, [], options);
```

## Custom cache path

Pass a directory path to the constructor to override the default `.jscpd/` location:

```typescript
// Cache stored at /tmp/my-jscpd-cache/<namespace>
const store: IStore = new LeveldbStore('/tmp/my-jscpd-cache');
```

Via CLI, use the `--store-path` option:

```bash
jscpd /path/to/source --store leveldb --store-path /tmp/my-jscpd-cache
```

### Parallel execution

When running multiple `jscpd` processes simultaneously, each process must use a distinct `storePath` to avoid LevelDB file conflicts:

```bash
jscpd /data/files/1 /data/repo/ --store leveldb --store-path /tmp/jscpd-1 --reporters json &
jscpd /data/files/2 /data/repo/ --store leveldb --store-path /tmp/jscpd-2 --reporters json &
wait
```


## License

[MIT](LICENSE) © Andrey Kucherenko

