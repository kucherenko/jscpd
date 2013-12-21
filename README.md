Copy/paste detector for JavaScript and CoffeeScript code
========================================================

`jscpd` is a tool for detect copy/past "design pattern" in JavaScript and CoffeeScript code.

Status
------
[![Dependency Status](https://gemnasium.com/mazerte/jscpd.png)](https://gemnasium.com/mazerte/jscpd)
[![Build Status](https://travis-ci.org/mazerte/jscpd.png?branch=master)](https://travis-ci.org/mazerte/jscpd)

Installation
------------

    npm install jscpd -g

Usage
-----

    jscpd -p /path/to/js/code -o /path/to/xml/output

or
	```coffeescript
	jscpd = require('jspd')
	result = jscpd::run
		path: 'my/project/folder'
	```


Options:
--------
 - -l, --min-lines        min size of duplication in code lines (Default is 5)
 - -t, --min-tokens       mim size of duplication in code tokens (Default is 70)
 - -p, --path             path to JavaScript code (Default is current working dir)
 - -c, --coffee BOOLEAN   is CoffeeScript code
 - -i, --ignore           directory to ignore
 - -o, --output           path to report xml file, report generated in PMD format
 - --verbose              show full info about copies
 - -v, --version          Display the current version
 - -h, --help             Display help and usage details