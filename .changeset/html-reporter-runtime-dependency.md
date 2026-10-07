---
"@jscpd/html-reporter": patch
---

Declare `@jscpd/finder` as a runtime dependency so importing the HTML reporter or jscpd works with isolated dependency resolution, including pnpm with hoisting disabled.
