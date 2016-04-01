#!/usr/bin/env bash

MOCHA=node_modules/mocha/bin/mocha
ISTANBUL=node_modules/.bin/istanbul

$MOCHA --recursive --compilers coffee:coffee-script/register --require coffee-coverage/register-istanbul $(find test -name '*.test.coffee')

$ISTANBUL report

#$COFFEECOVERAGE --path=relative ./src/ ./.tmp/
#COVERAGE=true $MOCHA \
#     --compilers coffee:coffee-script/register\
#     -R mocha-phantom-coverage-reporter $(find test -name '*.test.coffee')
