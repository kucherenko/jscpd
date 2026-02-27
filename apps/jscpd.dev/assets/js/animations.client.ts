// Client-side animations plugin for jscpd.dev
// All animations run in the browser only

export default defineNuxtPlugin(() => {
  // Wait for DOM to be ready
  if (typeof window !== 'undefined') {
    window.addEventListener('DOMContentLoaded', () => {
      // Small delay to ensure all elements are rendered
      setTimeout(() => {
        initScrollAnimations()
        initFloatingCodeBlocks()
        initReadingProgress()
      }, 100)
    })
  }
})

// Scroll Animations using Intersection Observer
function initScrollAnimations() {
  const observerOptions = {
    root: null,
    rootMargin: '0px',
    threshold: 0.1
  }

  const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
      if (entry.isIntersecting) {
        entry.target.classList.add('is-visible')
      }
    })
  }, observerOptions)

  // Observe all animated elements
  const animatedElements = document.querySelectorAll(
    '.animate-on-scroll, .slide-in-left, .slide-in-right, .scale-in, [data-animate]'
  )
  
  animatedElements.forEach((el, index) => {
    // Add staggered delay
    el.style.animationDelay = `${index * 0.1}s`
    observer.observe(el)
  })
}

// Floating Code Blocks for Hero Background
function initFloatingCodeBlocks() {
  const heroContainer = document.querySelector('.hero-container, .prose > .flex:first-child, .u-page-hero')
  
  if (!heroContainer) return
  
  // Check if floating code blocks already exist
  if (heroContainer.querySelector('.floating-code-bg')) return
  
  // Create floating code background
  const bgContainer = document.createElement('div')
  bgContainer.className = 'floating-code-bg'
  
  // Code snippets that represent duplication
  const codeSnippets = [
    'function duplicate() {',
    'const code = "copied"',
    '// TODO: refactor this',
    'if (copied) return true',
    'return duplicateCode',
    '// Found 3 clones',
    'const jscpd = require("jscpd")',
    'tokens: 180, lines: 42',
    'clone detected at line 10',
    'eslint-disable-copy',
    'git commit -m "fix copy/paste"',
    'searching for duplicates...'
  ]
  
  // Add floating code blocks
  for (let i = 0; i < 6; i++) {
    const codeBlock = document.createElement('div')
    codeBlock.className = 'floating-code-block'
    codeBlock.textContent = codeSnippets[i % codeSnippets.length]
    codeBlock.style.left = `${Math.random() * 80 + 10}%`
    codeBlock.style.top = `${Math.random() * 60 + 20}%`
    codeBlock.style.animationDelay = `${Math.random() * -20}s`
    codeBlock.style.fontSize = `${0.7 + Math.random() * 0.3}rem`
    bgContainer.appendChild(codeBlock)
  }
  
  heroContainer.style.position = 'relative'
  heroContainer.insertBefore(bgContainer, heroContainer.firstChild)
}

// Reading Progress Bar
function initReadingProgress() {
  // Only add on content pages (not home)
  if (typeof window !== 'undefined' && window.location.pathname !== '/' && window.location.pathname !== '') {
    // Create progress bar
    const progressBar = document.createElement('div')
    progressBar.className = 'reading-progress'
    document.body.appendChild(progressBar)
    
    // Update on scroll
    function updateProgress() {
      const windowHeight = window.innerHeight
      const documentHeight = document.documentElement.scrollHeight - windowHeight
      const scrolled = window.scrollY
      
      const progress = scrolled / documentHeight
      progressBar.style.transform = `scaleX(${progress})`
    }
    
    window.addEventListener('scroll', updateProgress, { passive: true })
    updateProgress() // Initialize
  }
}

// Typing Animation for Terminal (if present)
export function typeText(element, text, speed = 50) {
  let index = 0
  
  function type() {
    if (index < text.length) {
      element.textContent = text.substring(0, index + 1) + '|'
      index++
      setTimeout(type, speed)
    } else {
      element.textContent = text
    }
  }
  
  type()
}