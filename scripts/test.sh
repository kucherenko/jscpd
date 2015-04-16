#!/usr/bin/env bash

_MOCHA=node_modules/mocha/bin/mocha

_MOCHA --compilers coffee:coffee-script/register -R spec $(find test -name '*.test.coffee')
