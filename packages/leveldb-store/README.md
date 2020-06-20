# `@jscpd/leveldb-store`

> store for jscpd, used for large repositories

## Installation

```
npm install @jscpd/leveldb-store --save
```

## Usage

```
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

const store: IStore = new LeveldbStore();

const detector = new Detector(tokenizer, store, [], options);

```

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
