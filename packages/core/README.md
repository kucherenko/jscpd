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

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
