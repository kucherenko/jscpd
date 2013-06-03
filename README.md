Copy/paste detector for JavaScript code
=======================================

_jscpd_ is a tool for detect copy/past "design pattern" in javascript code.

Installation
------------

    npm install coffee-script -g

    npm install jscpd -g


Usage
-----

    jscpd -p /path/to/js/code -o /path/to/xml/output


Options:
--------
 - -m, --min-lines        min size of duplication in code lines (Default is 5)
 - -t, --min-tokens       mim size of duplication in code tokens (Default is 70)
 - -p, --path [PATH]      path to JavaScript code (Default is current working dir)
 - -i, --ignore PATH      directory to ignore
 - -l, --log PATH         path to log file
 - -o, --output PATH      path to report xml file
 - --verbose              show full info about copies
 - -v, --version          Display the current version
 - -h, --help             Display help and usage details