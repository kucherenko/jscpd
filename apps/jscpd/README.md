## jscpd

[![npm](https://img.shields.io/npm/v/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)
![jscpd](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/jscpd-badge.svg?sanitize=true)
[![license](https://img.shields.io/github/license/kucherenko/jscpd.svg?style=flat-square)](https://github.com/kucherenko/jscpd/blob/master/LICENSE)
[![npm](https://img.shields.io/npm/dw/jscpd.svg?style=flat-square)](https://www.npmjs.com/package/jscpd)


[![codecov](https://codecov.io/gh/kucherenko/jscpd/branch/master/graph/badge.svg)](https://codecov.io/gh/kucherenko/jscpd)
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2Fkucherenko%2Fjscpd?ref=badge_shield)
[![Backers on Open Collective](https://opencollective.com/jscpd/backers/badge.svg)](#backers)
[![Sponsors on Open Collective](https://opencollective.com/jscpd/sponsors/badge.svg)](#sponsors)

[![NPM](https://nodei.co/npm/jscpd.png)](https://nodei.co/npm/jscpd/)

> Copy/paste detector for programming source code, supports [223 formats](https://github.com/kucherenko/jscpd/blob/master/FORMATS.md). AI-ready with AI skills, MCP server and token-efficient reporter.

Copy/paste is a common technical debt on a lot of projects. The jscpd gives the ability to find duplicated blocks implemented on more than 223 programming languages and digital formats of documents.
The jscpd tool implements [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm) algorithm for searching duplications.

## Table of content

- [What's New](#whats-new)
- [Features](#features)
- [Getting started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
- [JSCPD Server](#jscpd-server)
- [Shebang Detection](#shebang-detection)
- [Options](#options)
  - [Formats Extensions](#formats-extensions)
  - [Formats Names](#formats-names)
- [Config File](#config-file)
- [Ignored Blocks](#ignored-blocks)
- [Reporters](#jscpd-reporters)
  - [HTML](#html)
  - [Badge](#badge)
  - [PMD CPD XML](#pmd-cpd-xml)
  - [JSON](#json-reporters)
- [API](#api)
- [Changelog](#changelog)
- [Who uses jscpd](#who-uses-jscpd)
- [Contributors](#contributors)
- [Backers](#backers)
- [Sponsors](#sponsors)
- [License](#license)


## Features
 - Detect duplications in programming source code, use semantic of programing languages, can skip comments, empty lines etc.
 - Detect duplications in embedded blocks of code, like `<script>` or `<style>` sections in html
 - Detect duplications in executable script files without extensions via [shebang detection](#shebang-detection)
 - Detect duplications in Svelte (`.svelte`), Astro (`.astro`), Vue SFC (`.vue`), and Markdown — tokenized per-block/per-section with cross-format duplicate detection across file types
 - Support for Apex, CFML/ColdFusion, and GDScript (Godot)
 - Blame authors of duplications
 - Generate XML report in pmd-cpd format, JSON report, [HTML report](http://kucherenko.github.io/jscpd-report.html)
 - Token-efficient `ai` reporter (~79% fewer tokens) for piping to LLM tools
 - Integrate with CI systems, use thresholds for level of duplications

## What's New

**v4.2.x**

- **Custom tokenizer backend** — replaced `prismjs` with an own backend built on the [reprism](https://github.com/tannerlinsley/reprism) grammar engine. ~11.5% faster tokenization on real projects (avg 1126ms → 997ms on a 548-file, 223-format scan).
- **Cross-format detection** — Vue SFC (`.vue`), Svelte (`.svelte`), Astro (`.astro`), and Markdown files are tokenized per-block/per-section, enabling duplicate detection across file types (e.g. a `<script>` block in `.vue` matched against `.ts` files).
- **New formats**: Apex, CFML/ColdFusion, GDScript, and 70+ additional formats (223 total, up from 152)
- **`--skipComments`**: shorthand flag for `--mode weak` (strip comments before detection)
- **Shebang detection**: auto-detect language for extensionless executable scripts
- **`--store-path`**: configure LevelDB cache directory for parallel runs
- **`--formats-names`**: map specific filenames (e.g. `Makefile`, `Dockerfile`) to a format
- **`--noTips`**: suppress tip output in CI environments

### Bug Fixes

- **Entire-file duplicates silently dropped** — RabinKarp flushed the pending clone on a store *hit* at end-of-file instead of on a *miss*, causing files that are complete copies of each other to go undetected. Fixed in `@jscpd/core` (#728).
- **ReDoS hang on Lisp/Elisp files** — the Lisp string regex could catastrophically backtrack (O(2ⁿ)) on unterminated strings. Replaced with a linear alternative in `@jscpd/tokenizer` (#737).
- **Process crash on malformed `package.json`** — invalid JSON in `package.json` threw an unhandled `SyntaxError` that killed the process. Now emits a warning and continues (#739).
- **Vue SFC cross-file detection broken** — the detector used the file-level format (`vue`) as the store namespace for all SFC blocks, preventing cross-file matches. Namespace now reflects each block's resolved sub-format.
- **Vue SFC incorrect column numbers** — tokens on the first line of a block carried block-relative column 1 instead of the file-absolute column.
- **50 dependency security vulnerabilities** remediated across the monorepo.

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
or

```bash
$ jscpd --pattern "src/**/*.js"
```

## JSCPD Server

If you need a standalone application that provides an API for detecting code duplication, you can use [jscpd-server](../jscpd-server).
It allows you to integrate duplication detection into your services or tools via HTTP API.

## Shebang Detection

jscpd can detect duplications in script files that have no file extension, such as shell scripts, Python scripts, or other executables deployed without an extension (e.g. `deploy`, `build`, `entrypoint`).

### How it works

When jscpd encounters a file with no recognized extension, it checks two conditions:

1. The file has the executable bit set (`chmod +x`)
2. The first line is a shebang (`#!...`)

If both conditions are met, jscpd reads the interpreter from the shebang line and maps it to a supported format.

### Supported interpreters

| Interpreter | Detected format |
|-------------|-----------------|
| `bash`, `sh`, `zsh`, `fish`, `dash`, `ksh` | `shell` |
| `python`, `python3`, `python2` | `python` |
| `node`, `nodejs` | `javascript` |
| `ruby` | `ruby` |
| `perl` | `perl` |
| `php` | `php` |
| `lua` | `lua` |
| `tclsh`, `wish` | `tcl` |
| `Rscript` | `r` |
| `groovy` | `groovy` |
| `swift` | `swift` |
| `kotlin` | `kotlin` |

Both direct (`#!/usr/bin/bash`) and `env`-mediated (`#!/usr/bin/env python3`) shebangs are supported. Version suffixes are stripped automatically (`python3.11` → `python`).

### Limitations

- Files without the executable bit are **not** inspected for shebangs and are skipped if they have no recognized extension — same behaviour as before.
- Symlinks are always excluded from shebang detection.
- If your interpreter is not in the table above, use [`--formats-names`](#formats-names) to map specific filenames to a format.

## Options
### Pattern

Glob pattern for find files to detect

 - Cli options: `--pattern`, `-p`
 - Type: **string**
 - Default: "**/*"

Example:
 ```bash
 $ jscpd --pattern "**/*.js"
 ```

### Min Tokens

Minimal block size of code in tokens. The block of code less than `min-tokens` will be skipped.

 - Cli options: `--min-tokens`, `-k`
 - Type: **number**
 - Default: **50**

 *This option is called ``minTokens`` in the config file.*

### Min Lines

Minimal block size of code in lines. The block of code less than `min-lines` will be skipped.

 - Cli options: `--min-lines`, `-l`
 - Type: **number**
 - Default: **5**
### Max Lines

Maximum file size in lines. The file bigger than `max-lines` will be skipped.

 - Cli options: `--max-lines`, `-x`
 - Type: **number**
 - Default: **1000**
### Max Size

Maximum file size in bytes. The file bigger than `max-size` will be skipped.

 - Cli options: `--max-size`, `-z`
 - Type: **string**
 - Default: **100kb**
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

Glob patterns for files and directories to exclude from analysis. Multiple patterns can be separated by commas.

 - Cli options: `--ignore`, `-i`
 - Type: **string**
 - Default: **null**

#### Pattern forms

All of the following forms work regardless of whether the scan path is relative or absolute:

| Pattern | Meaning |
|---------|---------|
| `**/patches/**` | ignore `patches` at any depth (already worked before v4.2.x) |
| `patches/**` | ignore `patches` relative to cwd or the scan directory |
| `./patches/**` | same as above, explicit `./` prefix |
| `/absolute/path/**` | ignore an absolute path |

#### Examples

```bash
# Ignore minified and map files anywhere in the tree
$ jscpd --ignore "**/*.min.js,**/*.map" /path/to/files

# Ignore a top-level directory when scanning cwd
$ jscpd --ignore "patches/**" .

# Ignore a subdirectory when scanning a subdirectory
# (pattern is resolved relative to the scanned path)
$ jscpd --ignore "./vendor/**" ./src

# Multiple patterns via comma separator
$ jscpd --ignore "dist/**,coverage/**,**/*.test.js" /path/to/project
```

In a config file the same patterns can be listed as an array:
```json
{
  "ignore": ["dist/**", "coverage/**", "**/__snapshots__/**"]
}
```

> **Note:** Patterns starting with `**/` match at any depth and are passed through unchanged. All other relative patterns are resolved against both cwd and each scan directory, so `patches/**` and `./patches/**` behave identically and work whether you scan `.`, an absolute path, or a subdirectory.
### Reporters
The list of reporters. Reporters use for output information of clones and duplication process.

Available reporters:
 - **console** - report about clones to console;
 - **ai** - compact, token-efficient clone list suited for piping to AI tools;
 - **consoleFull** - report about clones to console with blocks of code;
 - **json** - output `jscpd-report.json` file with clones report in json format;
 - **xml** - output `jscpd-report.xml` file with clones report in xml format;
 - **csv** - output `jscpd-report.csv` file with clones report in csv format;
 - **markdown** - output `jscpd-report.md` file with clones report in markdown format;
 - **html** - generate html report to `html/` folder;
 - **sarif** - generate a report in SARIF format (https://github.com/oasis-tcs/sarif-spec), save it to `jscpd-sarif.json` file;
 - **verbose** - output a lot of debug information to console;

> Note: A reporter can be developed manually, see [@jscpd/finder](../finder) package.

 - Cli options: `--reporters`, `-r`
 - Type: **string**
 - Default: **console**
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

### Skip Comments
Ignore comments during detection. Shorthand for `--mode weak`; comments are stripped before the duplicate-detection pass so comment-only blocks are never reported as clones.

If `--mode` is also provided, `--mode` takes precedence.

Example:
```bash
$ jscpd --skipComments /path/to/source
```

 - Cli options: `--skipComments`
 - Type: **boolean**
 - Default: **false**
### Format

The list of formats to detect for duplications. Available [223 formats](https://github.com/kucherenko/jscpd/blob/master/FORMATS.md).

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
### Ignore Case
Ignore case of symbols in code (experimental).


 - Cli options: `--ignoreCase`
 - Type: **boolean**
 - Default: **false**

### No Symlinks
Do not follow symlinks.

 - Cli options: `--noSymlinks`, `-n`
 - Type: **boolean**
 - Default: **false**

### Skip Local
Use for detect duplications in different folders only. For correct usage of `--skipLocal` option you should provide list of path's with more than one item.

Example:
```bash
jscpd --skipLocal /path/to/folder1/ /path/to/folder2/
```
will detect clones in separate folders only, clones from same folder will be skipped.


 - Cli options: `--skipLocal`
 - Type: **boolean**
 - Default: **false**

### Formats Extensions
Define the list of formats with file extensions. Available [223 formats](https://github.com/kucherenko/jscpd/blob/master/FORMATS.md).

In following example jscpd will analyze files `*.es` and `*.es6` as javascript and `*.dt` files as dart:
```bash
$ jscpd --formats-exts javascript:es,es6;dart:dt /path/to/code
```
> Note: formats defined in the option redefine default configuration, you should define all need formats manually or create two configuration for run `jscpd`

 - Cli options: `--formats-exts`
 - Type: **string**
 - Default: **null**

### Formats Names
Define the list of formats for files matched by exact filename (no extension required). This is independent of `--formats-exts` and does not affect extension-based detection.

Use this when you have extensionless files that are not covered by [shebang detection](#shebang-detection) — for example `Makefile`, `Dockerfile`, `Jenkinsfile`, or any script not starting with `#!/`.

```bash
$ jscpd --formats-names makefile:Makefile,GNUmakefile /path/to/code
$ jscpd --formats-names docker:Dockerfile;makefile:Makefile /path/to/code
```

The syntax mirrors `--formats-exts`: `format:name1,name2;format2:name3`.

 - Cli options: `--formats-names`
 - Type: **string**
 - Default: **null**

### Store

Stores used for collect information about code, by default all information collect in memory.

Available stores:
 - **leveldb** - leveldb store all data to files. The store recommended as store for big repositories. Should install @jscpd/leveldb-store before;

> Note: A store can be developed manually, see [@jscpd/finder](../finder) package and [@jscpd/leveldb-store](../leveldb-store) as example.

 - Cli options: `--store`
 - Type: **string**
 - Default: **null**

### Store Path

The directory used by the store for its cache files. By default, `--store leveldb` creates a `.jscpd/` directory in the current working directory. Use `--store-path` to override this location.

This is especially useful when running multiple `jscpd` processes in parallel — give each process a unique path to avoid LevelDB file conflicts:

```bash
# Two parallel runs, each with its own isolated cache
jscpd /data/files/1 /data/repo/ --store leveldb --store-path /tmp/jscpd-run1 --reporters json
jscpd /data/files/2 /data/repo/ --store leveldb --store-path /tmp/jscpd-run2 --reporters json
```

Can also be set in the config file:

```json
{
  "store": "leveldb",
  "storePath": "/tmp/my-jscpd-cache"
}
```

 - Cli options: `--store-path`
 - Type: **string**
 - Default: **`.jscpd`** (relative to current working directory)

### Ignore Pattern
Ignore code blocks matching the regexp patterns.

 - Cli options: `--ignore-pattern`
 - Type: **string**
 - Default: **null**

Example:
```
$ jscpd /path/to/source --ignore-pattern "import.*from\s*'.*'"
```
Excludes import statements from the calculation.

### No Tips

By default, jscpd prints a few tip lines after the timer output:

```
time: 1.234s

💡 Auto-refactor with AI: npx skills add kucherenko/jscpd --skill dry-refactoring
🎩 New: Gangsta Agents — discipline your AI coding → gangsta.page
💖 Sponsor jscpd → https://opencollective.com/jscpd
```

Use `--noTips` to suppress these lines (useful in CI environments or when piping output).

```bash
$ jscpd --noTips /path/to/source
```

Tips are also automatically suppressed when `--silent` is active.

 - Cli options: `--noTips`
 - Type: **boolean**
 - Default: **false**

## Config File

Put `.jscpd.json` file in the root of the projects:
```json
{
  "path": ["./src"],
  "threshold": 0,
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
    "path": ["./src"],
    "threshold": 0.1,
    "reporters": ["html", "console", "badge"],
    "ignore": ["**/__snapshots__/**"],
    "absolute": true,
    "gitignore": true
  }
  ...
}


```

### Exit code

By default, the tool exits with code 0 even when code duplications were
detected. This behaviour can be changed by specifying a custom exit
code for error states.

Example:
```bash
jscpd --exitCode 1 .
```

- Cli options: `--exitCode`
- Type: **number**
- Default: **0**


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
<!--
// jscpd:ignore-start
-->
<meta data-react-helmet="true" name="theme-color" content="#cb3837"/>
<link data-react-helmet="true" rel="stylesheet" href="https://static.npmjs.com/103af5b8a2b3c971cba419755f3a67bc.css"/>
<link data-react-helmet="true" rel="stylesheet" href="https://static.npmjs.com/cms/flatpages.css"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="120x120" href="https://static.npmjs.com/58a19602036db1daee0d7863c94673a4.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="144x144" href="https://static.npmjs.com/7a7ffabbd910fc60161bc04f2cee4160.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="152x152" href="https://static.npmjs.com/34110fd7686e2c90a487ca98e7336e99.png"/>
<link data-react-helmet="true" rel="apple-touch-icon" sizes="180x180" href="https://static.npmjs.com/3dc95981de4241b35cd55fe126ab6b2c.png"/>
<link data-react-helmet="true" rel="icon" type="image/png" href="https://static.npmjs.com/b0f1a8318363185cc2ea6a40ac23eeb2.png" sizes="32x32"/>
<!--
// jscpd:ignore-end
-->
```

## Reporters

### HTML

[Demo report](http://kucherenko.github.io/jscpd-report.html)
### Badge
![jscpd](../../assets/jscpd-badge.svg)

More info [jscpd-badge-reporter](https://github.com/kucherenko/jscpd-badge-reporter)
### AI

Compact, token-efficient reporter designed for piping jscpd output into AI tools.
Outputs one clone pair per line using common-path-prefix compression, followed by a summary.
No code fragments, no colors — clean for piping.

**Token savings: ~79% fewer tokens compared to the default console reporter.**

Benchmarked on the `fixtures/` directory (91 clones across 132 files):

| Reporter | Output size | Estimated tokens |
|----------|-------------|------------------|
| default (console) | ~21,800 chars | ~5,400 |
| `ai` | ~4,500 chars | ~1,100 |

Example output:
```
src/utils/ auth.ts:10-25 ~ helpers.ts:40-55
src/utils/auth.ts 30-45 ~ 80-95
src/ utils/auth.ts:10-25 ~ api/routes.ts:5-20
---
23 clones · 4.2% duplication
```

Activate with: `jscpd --reporters ai`

To use jscpd with an AI coding assistant, install the agent skills:

**jscpd** — tool reference skill (all CLI options, AI reporter format, config file syntax):
```bash
npx skills add kucherenko/jscpd --skill jscpd
```

**dry-refactoring** — guided refactoring workflow (read clones, choose strategy, apply refactor, verify):
```bash
npx skills add kucherenko/jscpd --skill dry-refactoring
```

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
  "duplicates": [{
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
    }
  }
}
```
## API


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

In case of deep customisation of detection process you can build your own tool:
If you are going to detect clones in file system you can use [@jscpd/finder](../finder) for make a powerful detector.
In case of detect clones in browser or not node.js environment you can build your own solution base on [@jscpd/code](../core)

## Changelog
[Changelog](CHANGELOG.md)

## Who uses jscpd
 - [Code-Inspector](https://www.code-inspector.com/) is a code analysis and technical debt management service.
 - [Mega-Linter](https://nvuillam.github.io/mega-linter/) is a 100% open-source linters aggregator for CI (GitHub Action & other CI tools) or to run locally
 - [vscode-jscpd](https://marketplace.visualstudio.com/items?itemName=paulhoughton.vscode-jscpd) VSCode Copy/Paste detector plugin.

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
