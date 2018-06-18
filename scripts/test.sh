#!/usr/bin/env bash

MOCHA=node_modules/mocha/bin/mocha

$MOCHA --require coffeescript/register -R spec $(find test -name '*.test.coffee')
