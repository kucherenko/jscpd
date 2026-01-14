---
seo:
  title: jscpd - Copy/Paste Detector for Source Code
  description: Detect copy/paste and duplicated code in your projects. Supports 150+ programming languages. Open source tool to reduce technical debt.
---

::u-page-hero
---
orientation: horizontal
---
#title
Copy/Paste Detector for Source Code

#description
**jscpd** hunts down duplicated blocks across **150+ languages** — because life's too short to maintain the same bug in five different places.

#default
<div class="relative bg-gradient-to-br from-primary/5 to-secondary/5 rounded-xl p-6 border border-primary/10 terminal-glow">
  <div class="flex items-center gap-3 mb-4">
    <div class="flex gap-2">
      <div class="w-3 h-3 rounded-full bg-red-400 animate-pulse"></div>
      <div class="w-3 h-3 rounded-full bg-yellow-400 animate-pulse" style="animation-delay: 0.2s"></div>
      <div class="w-3 h-3 rounded-full bg-green-400 animate-pulse" style="animation-delay: 0.4s"></div>
    </div>
    <span class="text-sm text-muted">Terminal</span>
  </div>
  
  <div class="font-mono text-sm">
    <div class="text-muted mb-2 typing-text">// Your code deserves better than copy/paste chaos</div>
    <div class="flex items-center gap-2 mb-2">
      <span class="text-green-400">$</span>
      <span class="text-blue-400">npx</span>
      <span class="text-primary font-semibold hero-gradient">jscpd</span>
      <span class="text-muted">./src</span>
      <span class="typing-cursor"></span>
    </div>
    <div class="text-muted" style="animation: fadeIn 0.5s ease 1s both;">→ Finding duplicates...</div>
    <div class="text-green-400 mt-2" style="animation: fadeIn 0.5s ease 2s both;">✓ Scan complete: 3 clones found</div>
  </div>
  
  <div class="absolute -bottom-2 -right-2 text-6xl opacity-10 animate-bounce" style="animation-duration: 3s;">🚀</div>
</div>

<style scoped>
.relative {
  position: relative;
}
.bg-gradient-to-br {
  background: linear-gradient(to bottom right, 
    rgba(var(--ui-color-primary-rgb), 0.05), 
    rgba(var(--ui-color-secondary-rgb), 0.05));
}
.border {
  border: 1px solid rgba(var(--ui-color-primary-rgb), 0.1);
}
</style>

#links
  :::u-button
  ---
  color: primary
  size: xl
  to: /getting-started/installation
  trailing-icon: i-lucide-arrow-right
  class: btn-glow
  ---
  Hunt Duplicates
  :::

  :::u-button
  ---
  color: neutral
  icon: simple-icons-github
  size: xl
  to: https://github.com/kucherenko/jscpd
  target: _blank
  variant: outline
  ---
  Star on GitHub
  :::

  :::u-button
  ---
  color: neutral
  size: xl
  to: https://www.npmjs.com/package/jscpd
  target: _blank
  variant: ghost
  icon: simple-icons-npm
  ---
  npm
  :::

  :::u-button
  ---
  color: neutral
  size: xl
  to: https://opencollective.com/jscpd
  target: _blank
  variant: ghost
  icon: i-lucide-heart
  ---
  Sponsor the project
  :::
::

::u-page-section
#title
Why Developers Love <span class="hero-gradient">jscpd</span>

#description
Because clean code is happy code

#features
  :::u-page-feature
  ---
  icon: i-lucide-award
  ---
  #title
  Since 2013
  
  #description
  A decade of refining the art of duplicate detection. Tried, tested, and trusted by thousands of teams worldwide.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-globe
  ---
  #title
  Speaks 150+ Languages
  
  #description
  JavaScript, Python, Java, Go, Rust, C++, TypeScript, Ruby... If you can write it, we can scan it. Even your YAML configs aren't safe.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-download
  ---
  #title
  20M+ Downloads
  
  #description
  One of the most trusted tools in the ecosystem. Join developers who rely on jscpd every day.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-rocket
  ---
  #title
  Blazingly Fast™
  
  #description
  Powered by the Rabin-Karp algorithm. Scans massive codebases before your coffee gets cold.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-terminal-square
  ---
  #title
  CLI-First Design
  
  #description
  One command to rule them all. Works everywhere — your laptop, CI/CD, that ancient Jenkins server nobody wants to touch.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-file-bar-chart
  ---
  #title
  Beautiful Reports
  
  #description
  HTML, JSON, XML, badges for your README. Make technical debt visible (and slightly embarrassing).
  :::

  :::u-page-feature
  ---
  icon: i-lucide-code-2
  ---
  #title
  Programmable
  
  #description
  Full API for Node.js. Build your own duplicate-detection empire. We won't judge.
  :::

  :::u-page-feature
  ---
  icon: i-lucide-shield-check
  ---
  #title
  CI/CD Ready
  
  #description
  Set a threshold, fail the build, save the day. Your future self will thank you.
  :::
::

::u-page-section
---
orientation: horizontal
---
#title
See It In Action

#description
From chaos to clarity in seconds

#default
```bash
# Scan your source code
$ jscpd ./src

Clone found (typescript):
 - src/utils.ts [10:1 - 25:3] (15 lines, 129 tokens)
   src/helpers.ts [5:1 - 20:3]

Clone found (typescript):
 - src/utils.ts [45:5 - 62:2] (17 lines, 178 tokens)
   src/components/Button.tsx [12:1 - 29:2]

Clone found (javascript):
 - src/hooks/useAuth.ts [1:1 - 34:2] (33 lines, 245 tokens)
   src/hooks/useSession.ts [1:1 - 34:2]

# ... more clones

Found 90 clones.
Detection time: 434.777ms
```
::

::u-page-section
---
orientation: horizontal
---
#title
Built by a Human Who Gets It

#description
Created with ❤️ by Andrey Kucherenko

#default
  :::u-card
  #default
  <div class="flex flex-col sm:flex-row items-center gap-6">
    <img 
      src="https://avatars.githubusercontent.com/kucherenko?v=4&size=128" 
      alt="Andrey Kucherenko" 
      class="w-24 h-24 rounded-full ring-4 ring-primary/20 shadow-xl flex-shrink-0 pointer-events-none"
      loading="lazy"
    >
    <div class="text-center sm:text-left">
      <p class="mb-4 text-muted">
        Andrey Kucherenko believes that every copy-pasted code block is a bug waiting to happen twice. 
        He built jscpd so you don't have to fix the same issue in five files.
      </p>
      <div class="flex flex-wrap justify-center sm:justify-start gap-3">
        <a href="https://github.com/kucherenko" target="_blank" class="inline-flex items-center gap-2 px-4 py-2 bg-muted hover:bg-muted/80 rounded-full text-sm font-medium transition-colors">
          <span class="i-simple-icons:github w-4 h-4"></span> GitHub
        </a>
        <a href="https://twitter.com/a_kucherenko" target="_blank" class="inline-flex items-center gap-2 px-4 py-2 bg-muted hover:bg-muted/80 rounded-full text-sm font-medium transition-colors">
          <span class="i-simple-icons:x w-4 h-4"></span> Twitter/X
        </a>
      </div>
    </div>
  </div>
  :::
::

::u-page-section
---
orientation: horizontal
reverse: true
---
#title
💙 Huge Thank You to Our Contributors!

#description
This project wouldn't exist without you

#default
  :::u-card
  #default
  **To everyone who has contributed to jscpd — thank you!** 🌟
  
  Whether you've submitted code, reported bugs, suggested features, improved documentation, or simply spread the word — your contributions make jscpd better for everyone. We're grateful for every issue closed, every PR merged, and every kind word shared.

  **With a grateful heart,** 🤗
  
  _The jscpd Team_
  
  [:icon{name="simple-icons-github" class="inline"} View Contributors](https://github.com/kucherenko/jscpd/graphs/contributors)
  :::
::
