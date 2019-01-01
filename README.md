![jscpd logo](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/logo.svg?sanitize=true)

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

> Copy/paste detector for programming source code, supports [150+ formats](docs/supported_formats.md).

Copy/paste is a common technical debt on a lot of projects. The jscpd gives the ability to find duplicated blocks implemented on more than 150 programming languages and digital formats of documents. 
The jscpd tool implements [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm for searching duplications.

[![jscpd screenshot](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/screenshot.png?sanitize=true)](http://kucherenko.github.io/jscpd-report.html)

## Table of content

- [Features](#features)
- [What is new in jscpd v1.0.0?](#what-is-new-in-jscpd-v100)
- [0.6.x](#06x)
- [Getting started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
- [Options](#options)
  - [Min Lines](#min-lines)
  - [Max Lines](#max-lines)
  - [Threshold](#threshold)
  - [Config](#config)
  - [Ignore](#ignore)
  - [Reporters](#reporters)
  - [Output](#output)
  - [Mode](#mode)
  - [Format](#format)
  - [Blame](#blame)
  - [Silent](#silent)
  - [Absolute](#absolute)
  - [Formats Extensions](#formats-extensions)
- [Config File](#config-file)
- [Ignored Blocks](#ignored-blocks)
- [JSCPD Reporters](#jscpd-reporters)
  - [HTML](#html)
  - [Badge](#badge)
  - [PMD CPD XML](#pmd-cpd-xml)
  - [JSON reporters](#json-reporters)
- [API](#api) ([Progamming API](docs/api.md))
- [Contributors](#contributors)
- [Backers](#backers)
- [Sponsors](#sponsors)
- [License](#license)


## Features
 - Detect duplications in programming source code, use semantic of programing languages, can skip comments, empty lines etc.
 - Detect duplications in embedded blocks of code, like `<script>` or `<style>` sections in html
 - Blame authors of duplications
 - Generate XML report in pmd-cpd format, JSON report, [HTML report](http://kucherenko.github.io/jscpd-report.html)

 - Integrate with CI systems, use thresholds for level of duplications 
 - The powerful [API](docs/api.md) for extend functionality and usage

## What is new in jscpd v1.0.0?

 - Powerful development [API](docs/api.md) written on TypeScript (no more CoffeeScript)
 - Supports more formats (moved source code tokenizer from CodeMirror to Prism.js)
 - New reporters: html, badge (badge reporter is separate package `jscpd-badge-reporter`)
 - Add blamed lines to JSON report
 - Default config file is `.jscpd.json`, no more `.cpd.yaml`
 - Detect different formats in one file, like js scripts in html tags
 - Allow to use multiple cli options for parameters like `jscpd --ignore tests,build`
 - Allow multiple paths for detection like `jscpd ./src ./tests ./docs`
 - Statistic of detection
 - Use patterns form `.gitignore` for ignoring detection
 - Ignored blocks in code 
 
## 0.6.x

The old version of jscpd [here](https://github.com/kucherenko/jscpd/tree/0.6.x)
 
## Getting started

### Installation
```bash
$ npm install -g jscpd
```
### Usage
```bash
$ npx jscpd /path/to/source
```
or 

```bash
$ jscpd /path/to/code
```
## Options
### Min Lines

Minimal block size of code in lines. The block of code less than `min-lines` will be skipped.
 
 - Cli options: `--min-lines`, `-l`
 - Type: **number**
 - Default: **5**
### Max Lines

Maximum file size in lines. The file bigger than `max-lines` will be skipped.
 
 - Cli options: `--max-lines`, `-x`
 - Type: **number**
 - Default: **500**
### Max Size

Maximum file size in bytes. The file bigger than `max-size` will be skipped.
 
 - Cli options: `--max-size`, `-z`
 - Type: **string**
 - Default: **30kb**
### Threshold

The threshold for duplication level, check if current level of duplications bigger than threshold jscpd exit with error.  

 - Cli options: `--threshold`, `-t`
 - Type: **number**
 - Default: **null**
### Config

The path to configuration file. The config should be in `json` format. Supported options in config file can be the same with cli options.

 - Cli options: `--config`, `-c`
 - Type: **path**
 - Default: **null** 
### Ignore

The option with glob patterns to ignore from analyze. For multiple globs you can use coma as separator.
Example:
```bash
$ jscpd --ignore "**/*.min.js,**/*.map" /path/to/files
```
 - Cli options: `--ignore`, `-i`
 - Type: **string**
 - Default: **null** 
### Reporters
The list of reporters. Reporters use for output information of clones and duplication process.

Available reporters:
 - **console** - report about clones to console;
 - **consoleFull** - report about clones to console with blocks of code;
 - **json** - output `jscpd-report.json` file with clones report in json format;
 - **xml** - output `jscpd-report.xml` file with clones report in xml format;
 - **html** - output `jscpd-report.html` file with clones report;
 - **verbose** - output a lot of debug information to console;
 - **time** - output all time of execution;

> Note: A reporter can be developed manually, see API section. 

 - Cli options: `--reporters`, `-r`
 - Type: **string**
 - Default: **console,time** 
### Output

The path to directory for reports. JSON and XML reports will be saved there.

 - Cli options: `--output`, `-o`
 - Type: **path**
 - Default: **./report/** 
 
### Mode
The mode of detection quality.
 - `strict` - use all types of symbols as token, skip only blocks marked as ignored.
 - `mild` - skip blocks marked as ignored and new lines and empty symbols.
 - `weak` - skip blocks marked as ignored and new lines and empty symbols and comments.

> Note: A mode can be developed manually, see API section.

 - Cli options: `--mode`, `-m`
 - Type: **string**
 - Default: **mild** 
### Format 

The list of formats to detect for duplications. Available over [150 formats](docs/supported_formats.md).

Example:
```bash
$ jscpd --format "php,javascript,markup,css" /path/to/files
```

 - Cli options: `--format`, `-f`
 - Type: **string**
 - Default: **{all formats}** 
### Blame
Get information about authors and dates of duplicated blocks from git.

 - Cli options: `--blame`, `-b`
 - Type: **boolean**
 - Default: **false** 
### Silent
Don't write a lot of information to a console.

Example:
```
$ jscpd /path/to/source --silent
Duplications detection: Found 60 exact clones with 3414(46.81%) duplicated lines in 100 (31 formats) files.
Execution Time: 1381.759ms
```
 - Cli options: `--silent`, `-s`
 - Type: **boolean**
 - Default: **false** 
### Absolute
Use the absolute path in reports.


 - Cli options: `--absolute`, `-a`
 - Type: **boolean**
 - Default: **false** 
 
### Formats Extensions
Define the list of formats with file extensions. Available over [150 formats](docs/supported_formats.md).

In following example jscpd will analyze files `*.es` and `*.es6` as javascript and `*.dt` files as dart:
```bash
$ jscpd --formats-exts javascript:es,es6;dart:dt /path/to/code 
```
> Note: formats defined in the option redefine default configuration, you should define all need formats manually or create two configuration for run `jscpd`  

 - Cli options: `--formats-exts`
 - Type: **string**
 - Default: **null** 
## Config File

Put `.jscpd.json` file in the root of the projects:
```json
{
  "threshold": 0.1,
  "reporters": ["html", "console", "badge"],
  "ignore": ["**/__snapshots__/**"],
  "absolute": true
}
```

Also you can use section in `package.json`:

```json
{
  ...
  "jscpd": {
    "threshold": 0.1,
    "reporters": ["html", "console", "badge"],
    "ignore": ["**/__snapshots__/**"],
    "absolute": true,
    "gitignore": true
  }
  ...
}


```
 
## Ignored Blocks

Mark blocks in code as ignored:
```javascript
/* jscpd:ignore-start */
import lodash from 'lodash';
import React from 'react';
import {User} from './models';
import {UserService} from './services';
/* jscpd:ignore-end */
```

```html
<!--jscpd:ignore-start-->
<meta data-react-helmet="true" name="theme-color" content="#cb3837"/>
<link data-react-helmet="true" rel="stylesheet" href="https://static.npmjs.com/103af5b8a2b3c971cba419755f3a67bc.css"/>
<link data-react-helmet="true" rel="stylesheet" href="https://static.npmjs.com/cms/flatpages.css"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="120x120" href="https://static.npmjs.com/58a19602036db1daee0d7863c94673a4.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="144x144" href="https://static.npmjs.com/7a7ffabbd910fc60161bc04f2cee4160.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="152x152" href="https://static.npmjs.com/34110fd7686e2c90a487ca98e7336e99.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="180x180" href="https://static.npmjs.com/3dc95981de4241b35cd55fe126ab6b2c.png"/>
<link data-react-helmet="true" rel="icon" type="image/png" href="https://static.npmjs.com/b0f1a8318363185cc2ea6a40ac23eeb2.png" sizes="32x32"/>
<!--jscpd:ignore-end-->
```
 
## JSCPD Reporters

### HTML

[Demo report](http://kucherenko.github.io/jscpd-report.html)
### Badge

![jscpd](assets/jscpd-badge.svg)

More info [jscpd-badge-reporter](https://github.com/kucherenko/jscpd-badge-reporter)
### PMD CPD XML
```xml
<?xml version="1.0" encoding="utf-8"?>
<pmd-cpd>
  <duplication lines="10">
      <file path="/path/to/file" line="1">
        <codefragment><![CDATA[ ...first code fragment... ]]></codefragment>
      </file>
      <file path="/path/to/file" line="5">
        <codefragment><![CDATA[ ...second code fragment...}]]></codefragment>
      </file>
      <codefragment><![CDATA[ ...duplicated fragment... ]]></codefragment>
  </duplication>
</pmd-cpd>
```
### JSON reporters
```json
{
  "duplications": [{
      "format": "javascript",
      "lines": 27,
      "fragment": "...code fragment... ",
      "tokens": 0,
      "firstFile": {
        "name": "tests/fixtures/javascript/file2.js",
        "start": 1,
        "end": 27,
        "startLoc": {
          "line": 1,
          "column": 1
        },
        "endLoc": {
          "line": 27,
          "column": 2
        }
      },
      "secondFile": {
        "name": "tests/fixtures/javascript/file1.js",
        "start": 1,
        "end": 24,
        "startLoc": {
          "line": 1,
          "column": 1
        },
        "endLoc": {
          "line": 24,
          "column": 2
        }
      }
  }],
  "statistic": {
    "detectionDate": "2018-11-09T15:32:02.397Z",
    "formats": {
      "javascript": {
        "sources": {
          "/path/to/file": {
            "lines": 24,
            "sources": 1,
            "clones": 1,
            "duplicatedLines": 26,
            "percentage": 45.33,
            "newDuplicatedLines": 0,
            "newClones": 0
          }
        },
        "total": {
          "lines": 297,
          "sources": 1,
          "clones": 1,
          "duplicatedLines": 26,
          "percentage": 45.33,
          "newDuplicatedLines": 0,
          "newClones": 0
        }
      }
    },
    "total": {
      "lines": 297,
      "sources": 6,
      "clones": 5,
      "duplicatedLines": 26,
      "percentage": 45.33,
      "newDuplicatedLines": 0,
      "newClones": 0
    },
    "threshold": 10
  }
}
```
## API

```typescript
import {
  JSCPD, 
  IClone,
  IOptions, 
  MATCH_SOURCE_EVENT, 
  CLONE_FOUND_EVENT,
  SOURCE_SKIPPED_EVENT,
  END_EVENT
} from 'jscpd';

const options: IOptions = {};

const cpd = new JSCPD(options);

const code = '...string with my code...';

cpd.on(MATCH_SOURCE_EVENT, (source) => {
  // new source detection started
  console.log(source);
});

cpd.on(CLONE_FOUND_EVENT, (clone: IClone) => {
  // clone found event
  console.log(clone);
});

cpd.on(SOURCE_SKIPPED_EVENT, (stat) => {
  // skipped source due size (see max-size, min-lines and max-lines options)
  console.log(stat);
});

cpd.on(END_EVENT, (clones: IClone[]) => {
  // detection process finished
  console.log(clones);
});

cpd.detect(code, { id: 'test', format: 'markup' })
  .then((clones: IClone[]) => console.log(clones));


cpd.detectInFiles(['./src', './tests'])
  .then((clones: IClone[]) => console.log(clones));

```

[Progamming API](docs/api.md)
## Contributors

This project exists thanks to all the people who contribute. 
<a href="https://github.com/kucherenko/jscpd/contributors"><img src="https://opencollective.com/jscpd/contributors.svg?width=890&button=false" /></a>
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
