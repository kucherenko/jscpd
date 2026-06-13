# `@jscpd/core`

> core package for detect duplicates, depends only on eventemitter3.

## Installation

```
npm install @jscpd/core --save
```

## Usage

```typescript
import {Tokenizer} from '@jscpd/tokenizer';
import {
    Detector,
    MemoryStore,
    IOptions,
    IClone,
    IStore,
    ITokenizer
} from '@jscpd/core';

const options: IOptions = {
    minLines: 5,
    maxLines: 500,
}

const tokenizer: ITokenizer = new Tokenizer();

// here you can use any store what implement IStore interface
const store: IStore = new MemoryStore();

// list of validators, implemented IValidator interface, validate clones
const validators = [];

const detector = new Detector(tokenizer, store, validators, options);

( async () => {
    const format = 'javascript';
    const code: string = '...string with code...';
    const clones: IClone[] = await detector.detect('source_id', code, format);

    console.log(clones);
})();

```


## License

[MIT](LICENSE) © Andrey Kucherenko
