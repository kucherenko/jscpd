
Copy/paste detector for programming source code.
============================================

`jscpd` is a tool for detect copy/paste "design pattern" in programming source code.

| _Supported languages_ |              |
|-----------------------|--------------|
| JavaScript            | Java         |
| CoffeeScript          | C++          |
| PHP                   | C#           |
| Go                    | Python       |
| Ruby                  | C            |
| Less                  | CSS          |
| SCSS                  | Mixed HTML   |
| TypeScript            | Haxe             |


If you need support language not from list feel free to create [request](https://github.com/kucherenko/jscpd/issues/new).

Status
------
[![Dependency Status](https://gemnasium.com/kucherenko/jscpd.png)](https://gemnasium.com/kucherenko/jscpd)
[![Build Status](https://travis-ci.org/kucherenko/jscpd.png?branch=master)](https://travis-ci.org/kucherenko/jscpd)
[![Coverage Status](https://coveralls.io/repos/kucherenko/jscpd/badge.png?branch=master)](https://coveralls.io/r/kucherenko/jscpd?branch=master)
[![Stories in Ready](https://badge.waffle.io/kucherenko/jscpd.png?label=ready)](https://waffle.io/kucherenko/jscpd)

[![Gitter](https://badges.gitter.im/Join Chat.svg)](https://gitter.im/kucherenko/jscpd?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

[![NPM](https://nodei.co/npm/jscpd.png)](https://nodei.co/npm/jscpd/)

Installation
------------

    npm install jscpd -g

Usage
-----

    jscpd --path my_project/ --languages javascript,coffee

    jscpd -f **/*.js -e **/node_modules/**

    jscpd --files **/*.js --exclude **/*.min.js --output report.xml

    jscpd --files **/*.js --exclude **/*.min.js --reporter json --output report.json

or

If you have file `.cpd.yaml` in your directory
```yaml
#.cpd.yaml
path:
  - fixtures/
languages:
  - javascript
  - coffeescript
  - typescript
  - php
  - python
  - jsx
  - haxe
  - css
  - ruby
  - go
  - java
  - clike    # c++ or c source
  - csharp      # c# source
  - htmlmixed   # html mixed source like knockout.js templates
exclude:
  - "**/*.min.js"
  - "**/*.mm.js"
reporter: json
```
and run `jscpd` command, you will check code for duplicates according config from .cpd.yaml

or

```coffeescript
# coffeescript
jscpd = require('jscpd')
result = jscpd::run
  path: 'my/project/folder'
  files: '**/*.js'
  exclude: ['**/*.min.js', '**/node_modules/**']
  reporter: json
```

Please see the [minimatch documentation](https://github.com/isaacs/minimatch) for more details.


Options:
--------

 Option             | Type      | Default       | Description
--------------------|-----------|---------------|-------------------------------------------------------------
 - -l, --min-lines  | [NUMBER]  | 5             | min size of duplication in code lines
 - -t, --min-tokens | [NUMBER]  | 70            | min size of duplication in code tokens
 - -f, --files      | [STRING]  | *             | glob pattern for find code
 - -r, --reporter   | [STRING]  | xml           | reporter name or path
 - -e, --exclude    | [STRING]  | -             | directory to ignore
 - -g, --languages  | [STRING]  | All supported | list of languages which scan for duplicates, separated with coma
 - -o, --output     | [PATH]    | -             | path to report file
 -     --verbose    |           | -             | show full info about copies
 - -p, --path       | [PATH]    | Current dir   | path to code
 - -d, --debug      |           | -             | show debug information (options list and selected files)
 - -v, --version    |           | -             | Display the current version
 - -h, --help       |           | -             | Display help and usage details

Reporters
---------

`jscpd` shipped with two standard reporters `xml` and [`json`](test/reporters/json-report.schema.json). It is possible to write custom reporter script too. For hooking reporter up wrap it into node module and provide path to it as `reporter` parameter e.g. `./scripts/jscpd-custom-reporter.coffee` (works with javascript too).

Custom reporter is a function which is executed into context of `Report` (`report.coffee`) class and thus has access to the report object and options. Expected output is array with following elements:

`[raw, dump, log]`

- `raw` is raw report object which will be passed through.
- `dump` is report which will be written into output file if any provided.
- `log` custom log output for cli.

At least one of `raw` or `dump` needs to be provided, `log` is fully optional.


Run tests
---------

```
  npm test
```

Changelog
---------

[Project changelog](https://github.com/kucherenko/jscpd/blob/master/changelog.md)

TODO
---------

[Project plans](https://github.com/kucherenko/jscpd/blob/master/todo.md)

License
-------

[The MIT License](https://github.com/kucherenko/jscpd/blob/master/LICENSE)

Thanks
------

Thanks to [Mathieu Desvé](https://github.com/mazerte) for [grunt-jscpd](https://github.com/mazerte/grunt-jscpd).
Thanks to [Yannick Croissant](https://yannick.cr/) for [gulp-jscpd](https://github.com/yannickcr/gulp-jscpd).
Thanks to [linslin](https://github.com/linslin) for [grunt-jscpd-reporter](https://github.com/linslin/grunt-jscpd-reporter).

Project developed with [PyCharm](http://www.jetbrains.com/pycharm/)
![alt pycharm](http://www.jetbrains.com/img/logos/pycharm_logo.gif)
Thanks to [JetBrains](http://www.jetbrains.com/) company for license key.
Feel free to contribute this project and you will have chance to get license key too.
