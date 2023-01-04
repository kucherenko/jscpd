# `@jscpd/finder`

> core package for detect duplicates, depends only on eventemitter3.

## Installation

```
npm install @jscpd/finder --save
```

## Usage

```typescript
import {Tokenizer} from '@jscpd/tokenizer';
import {
    MemoryStore,
    IOptions,
    IClone,
    IStore,
    ITokenizer
} from '@jscpd/core';
import {EntryWithContent, getFilesToDetect, InFilesDetector} from '@jscpd/finder';

const options: IOptions = {
    minLines: 5,
    maxLines: 500,
    path: ['list of folders and files to analyse for clones']
}

const tokenizer: ITokenizer = new Tokenizer();
// here you can use any store what implement IStore interface
const store: IStore = new MemoryStore();
const statistic = new Statistic(options);

const files: EntryWithContent[] = getFilesToDetect(options);

const detector = new InFilesDetector(tokenizer, store, statistic, options);

( async () => {
  const clones: IClone[] = await detector.detect(files);
})();
```


![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
