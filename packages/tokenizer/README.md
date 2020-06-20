# `@jscpd/tokenizer`

> tokenizer is package from @jscpd used for convert programming code to list of tokens


## Installation

```
npm install @jscpd/tokenizer --save
```

## Usage

```
import {IOptions,  ITokensMap} from '@jscpd/core';
import {Tokenizer} from '@jscpd/tokenizer';

const tokenizer = new Tokenizer();
const options: IOptions = {};

const maps: ITokensMap[] = tokenizer.generateMaps('source_id', 'let a = "11"', 'javascript', options);

```

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
