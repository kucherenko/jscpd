![jscpd logo](assets/logo.svg)


## jscpd
[![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
[![license](https://img.shields.io/github/license/kucherenko/jscpd.svg?style=flat-square)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![Travis](https://img.shields.io/travis/kucherenko/jscpd.svg?style=flat-square)](https://travis-ci.org/kucherenko/jscpd)
[![npm](https://img.shields.io/npm/dw/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
[![Coveralls](https://img.shields.io/coveralls/kucherenko/jscpd.svg?style=flat-square)](https://coveralls.io/github/kucherenko/jscpd)

> Copy/paste detector for programming source code, supports [150+ formats](docs/supported_formats.md).

Copy/paste is a common technical debt on a lot of projects. The jscpd gives the ability to find duplicated blocks implemented on more than 150 programming languages and digital formats of documents. 
The jscpd tool implements [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm for searching duplications.

[![NPM](https://nodei.co/npm/jscpd.png)](https://nodei.co/npm/jscpd/)

## Features
 - Detect duplications in programming source code, use semantic of programing languages, can skip comments, empty lines etc.
 - Detect duplications in embedded blocks of code, like `<script>` or `<style>` sections in html
 - Blame authors of duplications
 - Generate XML report in pmd-cpd format
 - Generate JSON report
 - Integrate with CI systems, use thresholds for level of duplications 
 - The powerful [API](docs/api.md) for extend functionality and usage
 
## Getting started

### Usage
```bash
$ npx jscpd@1.0.0-rc.3 /path/to/source
```

or 

```bash
$ npm install -g jscpd@1.0.0-rc.3

$ jscpd /path/to/code
```

### Options

#### Min Lines

Minimal block size of code in lines. The block of code less than `min-lines` will be skipped.
 
 - Cli options: `--min-lines`, `-l`
 - Type: **number**
 - Default: **5**
#### Max Lines

Maximum file size in lines. The file bigger than `max-lines` will be skipped.
 
 - Cli options: `--max-lines`, `-x`
 - Type: **number**
 - Default: **500**

#### Max Size

Maximum file size in bytes. The file bigger than `max-size` will be skipped.
 
 - Cli options: `--max-size`, `-z`
 - Type: **string**
 - Default: **50kb**
#### Threshold

The threshold for duplication level, check if current level of duplications bigger than threshold jscpd exit with error.  

 - Cli options: `--threshold`, `-t`
 - Type: **number**
 - Default: **null**

#### Config

The path to configuration file. The config should be in `json` format. Supported options in config file can be the same with cli options.

 - Cli options: `--config`, `-c`
 - Type: **path**
 - Default: **null** 

#### Ignore

The option with glob patterns to ignore from analyze. For multiple globs you can use coma as separator.
Example:
```bash
$ jscpd --ignore "**/*.min.js,**/*.map" /path/to/files
```
 - Cli options: `--ignore`, `-i`
 - Type: **string**
 - Default: **null** 

#### Reporters
The list of reporters. Reporters use for output information of clones and duplication process.

Available reporters:
 - **console** - report about clones to console;
 - **consoleFull** - report about clones to console with blocks of code;
 - **json** - output `jscpd-report.json` file with clones report in json format;
 - **xml** - output `jscpd-report.xml` file with clones report in xml format;
 - **verbose** - output a lot of debug information to console;
 - **time** - output all time of execution;

> Note: A reporter can be developed manually, see API section. 

 - Cli options: `--reporters`, `-r`
 - Type: **string**
 - Default: **console,time** 

#### Output

The path to directory for reports. JSON and XML reports will be saved there.

 - Cli options: `--output`, `-o`
 - Type: **path**
 - Default: **./report/** 
 
#### Mode
The mode of detection quality.
 - `strict` - use all types of symbols as token, skip only blocks marked as ignored.
 - `mild` - skip blocks marked as ignored and new lines and empty symbols.
 - `weak` - skip blocks marked as ignored and new lines and empty symbols and comments.

> Note: A mode can be developed manually, see API section.

 - Cli options: `--mode`, `-m`
 - Type: **string**
 - Default: **mild** 

#### Format 

The list of formats to detect for duplications. Available over [150 formats](docs/supported_formats.md).

Example:
```bash
$ jscpd --format "php,javascript,markup,css" /path/to/files
```

 - Cli options: `--format`, `-f`
 - Type: **string**
 - Default: **{all formats}** 

#### Blame
Get information about authors and dates of duplicated blocks from git.

 - Cli options: `--blame`, `-b`
 - Type: **boolean**
 - Default: **false** 

#### Silent
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

#### Absolute
Use the absolute path in reports.


 - Cli options: `--absolute`, `-a`
 - Type: **boolean**
 - Default: **false** 
 
#### Formats Extensions
Define the list of formats with file extensions. Available over [150 formats](docs/supported_formats.md).

In following example jscpd will analyze files `*.es` and `*.es6` as javascript and `*.dt` files as dart:
```bash
$ jscpd --formats-exts javascript:es,es6;dart:dt /path/to/code 
```
> Note: formats defined in the option redefine default configuration, you should define all need formats manually or create two configuration for run `jscpd`  

 - Cli options: `--formats-exts`
 - Type: **string**
 - Default: **null** 
 

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


![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

## License

[MIT](LICENSE) © Andrey Kucherenko
