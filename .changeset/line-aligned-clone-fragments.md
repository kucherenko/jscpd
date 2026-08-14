---
"@jscpd/finder": patch
---

Align clone fragments with the reported line numbers: a clone range can start mid-line and end just after a line terminator, so reporters rendered a truncated first line and a trailing empty line. The displayed fragment is now snapped to whole lines; token ranges, clone locations and statistics are unchanged. ([#916](https://github.com/kucherenko/jscpd/pull/916))
