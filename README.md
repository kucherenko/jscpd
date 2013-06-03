Copy/paste detector for JavaScript code
=======================================

Installation
------------

> npm install jscpd -g


Usage
-----

> jscpd -p /path/to/js/code -o /path/to/xml/output

Options:
  -m, --min-lines [NUMBER]min size of duplication in code lines (Default is 5)
  -t, --min-tokens [NUMBER]mim size of duplication in code tokens (Default is 70)
  -p, --path [PATH]      path to JavaScript code (Default is /home/apk/workspace/bmo/bmo-inv)
  -i, --ignore PATH      directory to ignore
  -l, --log PATH         path to log file
  -o, --output PATH      path to report xml file
      --verbose          show full info about copies
  -v, --version          Display the current version
  -h, --help             Display help and usage details