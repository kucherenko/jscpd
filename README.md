<p align="center">
  <img src="https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/logo.svg?sanitize=true">
</p>

## jscpd

[![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
![jscpd](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/jscpd-badge.svg?sanitize=true)
[![license](https://img.shields.io/github/license/kucherenko/jscpd.svg?style=flat-square)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![Travis](https://img.shields.io/travis/kucherenko/jscpd.svg?style=flat-square)](https://travis-ci.org/kucherenko/jscpd)
[![npm](https://img.shields.io/npm/dw/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)


[![codecov](https://codecov.io/gh/kucherenko/jscpd/branch/master/graph/badge.svg)](https://codecov.io/gh/kucherenko/jscpd)
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd?ref=badge_shield)
[![Backers on Open Collective](https://opencollective.com/jscpd/backers/badge.svg)](#backers)
[![Sponsors on Open Collective](https://opencollective.com/jscpd/sponsors/badge.svg)](#sponsors)

[![NPM](https://nodei.co/npm/jscpd.png)](https://nodei.co/npm/jscpd/)

> Copy/paste detector for programming source code, supports 150+ formats.

Copy/paste is a common technical debt on a lot of projects. The jscpd gives the ability to find duplicated blocks implemented on more than 150 programming languages and digital formats of documents.
The jscpd tool implements [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm for searching duplications.

[![jscpd screenshot](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/screenshot-1.png?raw=true)](http://kucherenko.github.io/jscpd-report.html)

## Packages of jscpd

| name                 | version  |  description  |
|----------------------|----------|---------------|
| [jscpd](packages/jscpd) | [![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd) | main package for jscpd (cli and API for detections included) |
| [@jscpd/core](packages/core) | [![npm](https://img.shields.io/npm/v/@jscpd/core.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/core) |core detection algorithm, can be used for detect duplication in different environments, one dependency to eventemmiter3 |
| [@jscpd/finder](packages/finder) | [![npm](https://img.shields.io/npm/v/@jscpd/finder.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/finder) | detector of duplication in files  |
| [@jscpd/tokenizer](packages/tokenizer) | [![npm](https://img.shields.io/npm/v/@jscpd/tokenizer.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/tokenizer) | tool for tokenize programming source code |
| [@jscpd/leveldb-store](packages/leveldb-store) | [![npm](https://img.shields.io/npm/v/@jscpd/leveldb-store.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/leveldb-store) | LevelDB store, used for big repositories, slower than default store |
| [@jscpd/html-reporter](packages/html-reporter) | [![npm](https://img.shields.io/npm/v/@jscpd/html-reporter.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd/html-reporter) | Html reporter for jscpd |

## Programming API

For integration copy/paste detection to your application you can use programming API:

`jscpd` Promise API
```typescript
import {IClone} from '@jscpd/core';
import {jscpd} from 'jscpd';

const clones: Promise<IClone[]> = jscpd(process.argv);
```

`jscpd` async/await API
```typescript
import {IClone} from '@jscpd/core';
import {jscpd} from 'jscpd';
(async () => {
  const clones: IClone[] = await jscpd(['', '', __dirname + '/../fixtures', '-m', 'weak', '--silent']);
  console.log(clones);
})();

```

`detectClones` API
```typescript
import {detectClones} from "jscpd";

(async () => {
  const clones = await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
    silent: true
  });
  console.log(clones);
})()
```

`detectClones` with persist store
```typescript
import {detectClones} from "jscpd";
import {IMapFrame, MemoryStore} from "@jscpd/core";

(async () => {
  const store = new MemoryStore<IMapFrame>();

  await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
  }, store);

  await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
    silent: true
  }, store);
})()
```

In case of deep customisation of detection process you can build your own tool with `@jscpd/core`, `@jscpd/finder` and `@jscpd/tokenizer`.

## Start contribution

 - Fork the repo [kucherenko/jscpd](https://github.com/kucherenko/jscpd/)
 - Clone forked version (`git clone https://github.com/{your-id}/jscpd`)
 - Install dependencies (`yarn install`)
 - Add your changes
 - Add tests and check it with `yarn test`
 - Create PR

## Who uses jscpd
 - [Code-Inspector](https://www.code-inspector.com/) is a code analysis and technical debt management service.
 - [vscode-jscpd](https://marketplace.visualstudio.com/items?itemName=paulhoughton.vscode-jscpd) VSCode Copy/Paste detector plugin.

## Backers

Thank you to all our backers! 🙏 [[Become a backer](https://opencollective.com/jscpd#backer)]

<a href="https://opencollective.com/jscpd#backers" target="_blank"><img src="https://opencollective.com/jscpd/backers.svg?width=890"></a>
## Sponsors

Support this project by becoming a sponsor. Your logo will show up here with a link to your website. [[Become a sponsor](https://opencollective.com/jscpd#sponsor)]

<a href="https://opencollective.com/jscpd/sponsor/0/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/0/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/1/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/1/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/2/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/2/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/3/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/3/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/4/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/4/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/5/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/5/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/6/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/6/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/7/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/7/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/8/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/8/avatar.svg"></a>
<a href="https://opencollective.com/jscpd/sponsor/9/website" target="_blank"><img src="https://opencollective.com/jscpd/sponsor/9/avatar.svg"></a>

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
