---
"@jscpd/finder": patch
---

Fix file discovery on Windows when paths use native separators. `fast-glob` treats a backslash as an escape character, so a scan path or ignore pattern containing `\` — whether typed by the user (`D:\project\src`) or produced by `path.join()` / `path.resolve()` — silently matched nothing and jscpd reported no files. Separators are now normalized to `/` before patterns reach `fast-glob`; POSIX behaviour is unchanged, since a backslash is a legal filename character there. ([#602](https://github.com/kucherenko/jscpd/issues/602))
