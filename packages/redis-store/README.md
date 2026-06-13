# `@jscpd/redis-store`

> Redis-backed token store for [jscpd](https://github.com/kucherenko/jscpd) — offloads the in-memory hash map to a Redis instance, useful for large codebases or distributed / CI environments where memory is constrained.

## Installation

```bash
npm install @jscpd/redis-store --save
```

## Usage

```typescript
import { Tokenizer } from '@jscpd/tokenizer';
import {
  Detector,
  IOptions,
  IClone,
  IStore,
  ITokenizer,
} from '@jscpd/core';
import RedisStore from '@jscpd/redis-store';

const options: IOptions = {
  minLines: 5,
  maxLines: 500,
};

const tokenizer: ITokenizer = new Tokenizer();
const store: IStore = new RedisStore();

const detector = new Detector(tokenizer, store, [], options);
```


## License

[MIT](LICENSE) © Andrey Kucherenko
