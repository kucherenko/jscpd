fs = require 'fs'
logger = require 'winston'


class Report

  constructor: (@options) ->

  generate: (@map) ->

    result = "Found #{map.clones.length} exact clones with
    #{map.numberOfDuplication} duplicated lines in
    #{map.numberOfFiles} files\n #{result}"

    logger.info "#{result}\n\n
    #{map.getPercentage()}% (#{map.numberOfDuplication} lines)
    duplicated lines out of
    #{map.numberOfLines} total lines of code.\n"

    return xmlDoc

exports.Report = Report
