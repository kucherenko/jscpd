#!/usr/bin/env bash

MOCHA=node_modules/mocha/bin/mocha

$MOCHA --compilers coffee:coffee-script/register -R spec $(find test -name '*.test.coffee')
