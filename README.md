Copy/paste detector for JavaScript code
=======================================

`jscpd` is a tool for detect copy/past "design pattern" in javascript code.

Installation
------------

    npm install jscpd -g

Usage
-----

    jscpd -p /path/to/js/code -o /path/to/xml/output


Options:
--------
 - -l, --min-lines        min size of duplication in code lines (Default is 5)
 - -t, --min-tokens       mim size of duplication in code tokens (Default is 70)
 - -p, --path             path to JavaScript code (Default is current working dir)
 - -i, --ignore           directory to ignore
 - -o, --output           path to report xml file, report generated in PMD format
 - --verbose              show full info about copies
 - -v, --version          Display the current version
 - -h, --help             Display help and usage details