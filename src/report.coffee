fs = require 'fs'
logger = require 'winston'


class Report

  constructor: (@options) ->

    @reporter = require @options.reporter

  generate: (@map) ->

    [report, log] = @reporter(@map, @options)


    log = "Found #{@map.clones.length} exact clones with
        #{@map.numberOfDuplication} duplicated lines in
        #{@map.numberOfFiles} files\n #{log}"

    logger.info "#{log}\n\n
        #{@map.getPercentage()}% (#{@map.numberOfDuplication} lines)
        duplicated lines out of
        #{@map.numberOfLines} total lines of code.\n"

    fs.writeFileSync(@options.output, report) if @options.output

    return report

exports.Report = Report
