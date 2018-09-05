#!/usr/bin/env bash

COFFEELINT=node_modules/coffeelint/bin/coffeelint

$COFFEELINT $(find test src -name '*.coffee')
