# `@jscpd/finder`

> File discovery and in-files detection package for [jscpd](https://github.com/kucherenko/jscpd) — walks the filesystem to collect source files, runs the duplicate detector across them, and provides built-in reporters, subscribers, validators, and hooks.

Key exports:

- **`getFilesToDetect(options)`** — resolves the file list from paths, globs, and ignore patterns
- **`InFilesDetector`** — orchestrates tokenization, store lookup, and clone collection across a file list
- **Reporters** — `console`, `consoleFull`, `json`, `xml`, `csv`, `markdown`, `xcode`, `ai`, `silent`, `threshold`
- **Subscribers** — `verbose`, `progress`
- **Validators** — `skipLocal`
- **Hooks** — `blamer`, `fragment`

## Installation

```bash
npm install @jscpd/finder --save
```

## Usage

```typescript
import { Tokenizer } from '@jscpd/tokenizer';
import {
  MemoryStore,
  IOptions,
  IClone,
  IStore,
  ITokenizer,
} from '@jscpd/core';
import { EntryWithContent, getFilesToDetect, InFilesDetector } from '@jscpd/finder';

const options: IOptions = {
  minLines: 5,
  maxLines: 500,
  path: ['list of folders and files to analyse for clones'],
};

const tokenizer: ITokenizer = new Tokenizer();
// any store implementing IStore works here
const store: IStore = new MemoryStore();

const files: EntryWithContent[] = getFilesToDetect(options);
const detector = new InFilesDetector(tokenizer, store, options);

(async () => {
  const clones: IClone[] = await detector.detect(files);
})();
```

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
