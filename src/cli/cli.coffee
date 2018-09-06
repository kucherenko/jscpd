logger = require 'winston'
cli = require("cli").enable "help", "version"
path = require "path"
JsCpd = require "./../jscpd"

logger.cli()

cli.setUsage "jscpd [OPTIONS]"
cli.setApp path.resolve "#{__dirname}/../../package.json"

cli.parse {
  "min-lines": [
    'l'
    "min size of duplication in code lines"
    "number"
  ]
  "min-tokens": [
    't'
    "min size of duplication in code tokens"
    "number"
  ]
  "config": [
    'c'
    "path to config file"
    "file"
  ]
  "files": [
    'f'
    "glob pattern for find code"
    "string"
  ]
  "exclude": [
    'e'
    "directory to ignore"
    "string"
  ]
  "skip-comments": [
    off
    "skip comments in code"
  ]
  "blame": [
    'b'
    "blame authors of duplications (get information about authors from git)"
    "boolean"
  ]
  "languages-exts": [
    off
    "list of languages with file extensions (language:ext1,ext2;language:ext3)"
    "string"
  ]
  "languages": [
    'g'
    "list of languages which scan for duplicates, separated with comma"
    "string"
  ]
  "output": [
    'o'
    "path to report file"
    "path"
  ]
  "reporter": [
    'r'
    "reporter to use"
    "string"
  ]
  "xsl-href": [
    'x'
    "path to xsl for include to xml report"
    "string"
  ]
  "verbose": [
    off
    "show full info about copies"
  ]
  "debug": [
    'd'
    "show debug information(options list and selected files)"
  ]
  "path": [
    'p'
    "path to code"
    "path"
  ]
  "limit": [
    off
    'limit of allowed duplications, if real duplications percent more than limit jscpd exit with error'
    "float"
  ]
  "silent": [
    's'
    "do not write list of files with duplicates to console"
    "boolean"
  ]
}

cli.main (args, options) ->
  jscpd = new JsCpd
  logger.profile "All time:"
  logger.info """
jscpd - copy/paste detector for programming source code, developed by Andrey Kucherenko
"""
  jscpd.run options
  logger.profile "All time:"
