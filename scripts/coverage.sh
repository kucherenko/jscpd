#!/usr/bin/env bash

MOCHA=node_modules/mocha/bin/mocha
ISTANBUL=node_modules/.bin/istanbul

$MOCHA --recursive --require coffeescript/register --require coffee-coverage/register-istanbul $(find test -name '*.test.coffee')

$ISTANBUL report
