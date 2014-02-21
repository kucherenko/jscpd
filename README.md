Copy/paste detector for programming source code.
============================================

`jscpd` is a tool for detect copy/past "design pattern" in programming source code.

| _Supported languages_                |
|--------------------------------------|
| JavaScript            | Java         |
| CoffeeScript          | C++          |
| PHP                   | C#           |
| Less                  | Python       |
| Ruby                  | C            |




Status
------
[![Dependency Status](https://gemnasium.com/kucherenko/jscpd.png)](https://gemnasium.com/kucherenko/jscpd)
[![Build Status](https://travis-ci.org/kucherenko/jscpd.png?branch=master)](https://travis-ci.org/kucherenko/jscpd)
[![Coverage Status](https://coveralls.io/repos/kucherenko/jscpd/badge.png?branch=master)](https://coveralls.io/r/kucherenko/jscpd?branch=master)
[![Bitdeli Badge](https://d2weczhvl823v0.cloudfront.net/kucherenko/jscpd/trend.png)](https://bitdeli.com/free "Bitdeli Badge")

[![NPM](https://nodei.co/npm/jscpd.png?downloads=true)](https://nodei.co/npm/jscpd/)

Installation
------------

    npm install jscpd -g

Usage
-----

    jscpd --path my_project/ --languages js,coffee #scan for js and coffee files for duplicates

    jscpd -f **/*.js -e **/node_modules/**

    jscpd --files **/*.js --exclude **/*.min.js --output report.xml

or

If you have file `.cpd.yaml` in your directory
```yaml
#.cpd.yaml
path:
  - fixtures/
languages:
  - javascript
  - coffeescript
  - php
  - python
  - less
  - ruby
  - java
  - "c++src" # c++ source
  - csrc     # c source
  - csharp   # c# source
exclude:
  - "**/*.min.js"
  - "**/*.mm.js"
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
```

Please see the [minimatch documentation](https://github.com/isaacs/minimatch) for more details.

Deprecated style:

    jscpd --ignore node_modules/ --coffee


Options:
--------

 Option             | Type      | Default       | Description
--------------------|-----------|---------------|-------------------------------------------------------------
 - -l, --min-lines  | [NUMBER]  | 5             | min size of duplication in code lines
 - -t, --min-tokens | [NUMBER]  | 70            | mim size of duplication in code tokens
 - -f, --files      | [STRING]  | *             | glob pattern for find code
 - -e, --exclude    | [STRING]  | -             | directory to ignore
 - -g, --languages  | [STRING]  | All supported | list of languages which scan for duplicates, separated with coma
 - -o, --output     | [PATH]    | -             | path to report xml file
 -     --verbose    |           | -             | show full info about copies
 - -p, --path       | [PATH]    | Current dir   | path to code
 - -d, --debug      |           | -             | show debug information (options list and selected files)
 - -v, --version    |           | -             | Display the current version
 - -h, --help       |           | -             | Display help and usage details

Run tests
---------

```
  npm test
```

License
-------

[The MIT License](https://github.com/kucherenko/jscpd/blob/master/LICENSE)