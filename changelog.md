0.6.0 / 2016-03-31
==================

  * documentation(readme) update options section
  * documentation(screenshots) add screenshots for verbose and blame modes
  * feature(blame) add blame author of duplication as cli option
  * feature(blame) add blame author of duplication
  * feature(preprocessor) add debug preprocessor
    feractor(jscpd) add preproccessors support
  * feature(preprocessor) add files preprocessor
  * feature(preprocessor) update options preprocessor
  * feature(preprocessor) add options preprocessor
  * improve console output for verbose mode (colors and formating)

0.5.4 / 2016-03-09
==================

  * add new feature for FR [#48](https://github.com/kucherenko/jscpd/issues/48), add limit for duplications
  * Merge pull request [#53](https://github.com/kucherenko/jscpd/issues/53) from skyl/feature/issue49-hash-collision
    Feature/issue49 hash collision
  * fixes [#49](https://github.com/kucherenko/jscpd/issues/49) hash collisions with large project - birthday paradox
  * fixes [#50](https://github.com/kucherenko/jscpd/issues/50) and [#52](https://github.com/kucherenko/jscpd/issues/52) with passing tests
  * fixes [#52](https://github.com/kucherenko/jscpd/issues/52), config.path works mostly
  * fixes [#50](https://github.com/kucherenko/jscpd/issues/50), path from config file

0.5.3 / 2015-12-14
==================

  * Update version and add information about YAML support to README
  * Merge remote-tracking branch 'origin/master'
  * Merge pull request [#46](https://github.com/kucherenko/jscpd/issues/46) from SebastienElet/feature/yaml
    feat(language): add yaml support
  * feat(language): add yaml support
    Add yaml support with two ansible files as testing fixtures
  * fix build, add development version of bin script
  * add blamer
  * update config for test
  * update todo list, changelog and npmignore

0.5.2 / 2015-11-08
==================

  * Merge pull request [#45](https://github.com/kucherenko/jscpd/issues/45) from kucherenko/develop
    add more options for detections, added tests for ruby, csharp and java
  * update version and add lines to .npmignore
  * add more tests for csharp, java and ruby
  * add option for skip comments in source
  * skip empty symbols in diffs, add more information to xml report
  * Merge pull request [#44](https://github.com/kucherenko/jscpd/issues/44) from kucherenko/develop
    Develop
  * Merge branch 'master' of github.com:kucherenko/jscpd into develop

0.5.1 / 2015-11-02
==================

  * Remove coffeescript from dependencies (fix for [#42](https://github.com/kucherenko/jscpd/issues/42))
  * change license field

0.5.0 / 2015-11-01
==================

  * Add suport xsl for xml reporter (issue [#35](https://github.com/kucherenko/jscpd/issues/35)) and change priority of options (issue [#21](https://github.com/kucherenko/jscpd/issues/21)), updated version to 0.5.0

0.4.3 / 2015-11-01
==================

  * Merge branch 'master' of github.com:kucherenko/jscpd
  * Update version
  * Add *.es and *.es6 support (Fix for bug [#41](https://github.com/kucherenko/jscpd/issues/41))
  * Merge pull request [#40](https://github.com/kucherenko/jscpd/issues/40) from lo1tuma/compile-coffee-before-publishing
    Compile coffee before publishing to npm (fixes [#39](https://github.com/kucherenko/jscpd/issues/39))
  * Compile coffee before publishing to npm (fixes [#39](https://github.com/kucherenko/jscpd/issues/39))
  * Update version and fix build

0.4.2 / 2015-09-10
==================

  * Fixed invalid yaml config upgrading from 0.4.0 to 0.4.1 [#37](https://github.com/kucherenko/jscpd/issues/37)

0.4.1 / 2015-09-08
==================

  * Updated README
  * Added support of Haxe [#32](https://github.com/kucherenko/jscpd/issues/32)
  * Added support for JSX files [#29](https://github.com/kucherenko/jscpd/issues/29)
  * Merge pull request [#33](https://github.com/kucherenko/jscpd/issues/33) from istro/spelling
    Add a second n to scan*n*ing
  * :pencil: add a second n to scan*n*ing
  * Merge pull request [#31](https://github.com/kucherenko/jscpd/issues/31) from golmansax/master
    Allow for .cpd.yml in addition to .cpd.yaml
  * Allow for .cpd.yml in addition to .cpd.yaml

0.4.0 / 2015-05-03
==================

  * Merge pull request [#27](https://github.com/kucherenko/jscpd/issues/27) from dmi3y/custom-reporters
    Custom reporters
  * yaml config example correction
  * absolute path check
  * readme tweaks
  * removing double line
  * relative path aproach
  * Merge pull request [#25](https://github.com/kucherenko/jscpd/issues/25) from dmi3y/custom-reporters
    Custom reporters
  * 0.4.0
  * Readme and changelog
  * merge conflict
  * more about custom reporters
  * readme and reporters api tweaks
  * tests tweaks
  * json validaton schema tweaks
  * custom reporters
  * Merge branch 'master' of https://github.com/kucherenko/jscpd into custom-reporters
  * reporters separation
  * moving reporters tests
  * Merge pull request [#24](https://github.com/kucherenko/jscpd/issues/24) from dmi3y/tests-windows
    Tests for Windows
  * coffeelint tweaks
  * editorconfig
  * standard log busted
  * destignated std log and other fixes
  * conflict resolution
  * reporters folder
    mocking up with reporters
  * arch test
  * variables
  * scripts folder
  * for windows users npm run test
  * splitting out reporters
  * json reporter
  * mocking up with reporters
  * reporters folder
  * Merge pull request [#23](https://github.com/kucherenko/jscpd/issues/23) from darthwade/patch-1
    Update package.json
  * Update package.json
  * Merge pull request [#19](https://github.com/kucherenko/jscpd/issues/19) from Gillespie59/patch-1
    Mispelling in Readme.md
  * Mispelling in Readme.md
  * fix build
  * Added Typescript support [#14](https://github.com/kucherenko/jscpd/issues/14)

0.3.6 / 2015-02-28
==================

  * Merge pull request [#17](https://github.com/kucherenko/jscpd/issues/17) from JeremyDurnell/master
    Added support for htmlmixed languages (e.g. knockoutjs)
  * fixing white space issue with htmlmixed test files
  * added support for htmlmixed
  * Merge pull request [#16](https://github.com/kucherenko/jscpd/issues/16) from titarenko/patch-1
    Make error message more clear
  * Make error message more clear
  * updated dependencies

0.3.5 / 2015-02-04
==================

  * updated dependencies

0.3.4 / 2015-02-04
==================

  * update readme, removed non neede badges
  * update readme, big thank you for gulp-jscpd & grunt-jscpd-reporter
  * Added css support
  * Implement task [#13](https://github.com/kucherenko/jscpd/issues/13) (added go language support, added css/sccs support)
  * updated doc
  * Merge branch 'master' of github.com:kucherenko/jscpd
  * fix for [#12](https://github.com/kucherenko/jscpd/issues/12) , updated docs and changed minor version
  * Merge pull request [#10](https://github.com/kucherenko/jscpd/issues/10) from gitter-badger/gitter-badge
    Add a Gitter chat badge to README.md
  * Added Gitter badge

0.3.3 / 2014-09-22
==================

  * fix for [#8](https://github.com/kucherenko/jscpd/issues/8)
  * Fix tests, changed configuration of jscpd result
  * Merge pull request [#9](https://github.com/kucherenko/jscpd/issues/9) from ffesseler/return-code-map
    Return report & code map results instead of just report
  * Return report & code map results instead of just report
  * Merge pull request [#7](https://github.com/kucherenko/jscpd/issues/7) from eavgerinos/patch-1
    Fix typo 'past' -> 'paste'
  * Fix typo 'past' -> 'paste'
  * Fix travis configuration
  * Changed CI travis configuration
  * Added readme and information about PyCharm
  * changed versions of dependensies
  * refactored tests
  * Merge pull request [#5](https://github.com/kucherenko/jscpd/issues/5) from waffleio/master
    waffle.io Badge
  * add waffle.io badge

0.3.2 / 2014-02-17
==================

  * remove deprecated options
  * added profile points, added debug option
  * added logger and updated underscore
  * added winston logger to dependencies
  * Add information about changes

0.3.1 / 2014-02-07
==================

  * Add php support

0.3.0 / 2014-02-07
==================

  * Exclude node_modules from cpd process
  * Added information about supported languages
  * Add .cpd.yaml and task for travis for duplicate detection
  * Add less language support
  * Fix package.json
  * Changed changelog :)
  * fix tests, added changelog
  * wip, remove js tokenizer
  * wip, change tokenizer

0.2.19 / 2014-01-20
===================

  * Added license field to package.json
  * Added license (issue [#4](https://github.com/kucherenko/jscpd/issues/4))
  * fix build

0.2.18 / 2013-12-26
===================

  * fix bug with path

0.2.17 / 2013-12-26
===================

  * fix bug with path to config

0.2.16 / 2013-12-26
===================

  * added yaml config support

0.2.15 / 2013-12-26
===================

  * changed algorithm of tokenizer and options parsing
  * fix broken tests
  * refactoring
  * fix failed test, update version to 0.2.15
  * added documentation about parameters

0.2.14 / 2013-12-24
===================

  * fix bug with empty excludes

0.2.13 / 2013-12-24
===================

  * changed algorithm for files search
    changed version of jscpd to 0.2.13
  * fix readme
  * Added NPM label
  * Add a Bitdeli badge to README

0.2.12 / 2013-12-23
===================

  * update version of jscpd
  * Merge pull request [#3](https://github.com/kucherenko/jscpd/issues/3) from bitdeli-chef/master
    Add a Bitdeli Badge to README
  * Merge pull request [#2](https://github.com/kucherenko/jscpd/issues/2) from mazerte/master
    Add index.js

0.2.11 / 2013-12-22
===================

  * changed version of package
  * added main field to package.json
  * Add index import in test
  * Add index.js
  * added new fields to npmignore

0.2.10 / 2013-12-21
===================

  * changed travis configuration
  * Changed settings for travice and coveralls
    Changed version os jscpd after megre
  * Merge pull request [#1](https://github.com/kucherenko/jscpd/issues/1) from mazerte/master
    Add unit-test and Node api
  * Update reporter version
  * Fix build
  * Fix travis conf
  * Fix readme
  * Merge
  * Add Coveralls badge
  * Add Coverage
  * Update Readme for nodejs usage
  * Fix badge mistacke
  * Update dependencies and add badges
  * Add unit-test and js api

0.2.9 / 2013-10-10
==================

  * Removed jshint dependencies
  * Added tests

0.2.8 / 2013-09-24
==================

  * Fixed bug with wrong file names

0.2.7 / 2013-09-24
==================

  * Remove jshint from dependencies, added js tokenizer

0.2.6 / 2013-09-17
==================

  * Changed dependencies and npmignore

0.2.3 / 2013-09-17
==================

  * Changing coffee according coffeelint
    Added npmignore
  * Update version

0.2.0 / 2013-09-16
==================

  * Updated README.md and package information
  * Merge remote-tracking branch 'origin/master'
  * Added support for CoffeeScript code

0.1.7 / 2013-06-20
==================

  * Changed bin/jscpd file

0.1.6 / 2013-06-03
==================

  * Changed cli options & description & version
  * Changed description
  * Removed jade dependency
    Added description about coffee-script
  * Changed markup

0.1.5 / 2013-06-03
==================

  * Added short readme
  * Added engines section in package.json
  * changed xml version

0.1.3 / 2013-06-03
==================

  * Changed version of app
  * Fixed error with package.json

0.1.2 / 2013-06-03
==================

  * Fixed error with bin

0.1.1 / 2013-06-03
==================

  * Refactored package.json
  * Added xml report
  * Added reporter
  * Added first implementation of copy/past detection
    Added fixtures
  * Added token extraction to Strategy
  * First view on jscpd
  * initial commit
