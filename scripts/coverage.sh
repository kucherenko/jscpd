#!/usr/bin/env bash

MOCHA=node_modules/mocha/bin/mocha
COFFEECOVERAGE=node_modules/coffee-coverage/bin/coffeecoverage
COVERAGE=true

mkdir ./.tmp/
mkdir ./.tmp/tokenizer
$COFFEECOVERAGE --path=relative ./src/ ./.tmp/
$MOCHA \
     --compilers coffee:coffee-script/register\
     -R mocha-phantom-coverage-reporter $(find test -name '*.test.coffee')
