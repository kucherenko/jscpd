#!/usr/bin/env bash

mocha --compilers coffee:coffee-script/register -R spec $(find test -name '*.test.coffee')