// @ts-check
'use strict'

const fs = require('fs')
const path = require('path')

// gray-matter lives in the workspace root node_modules
let matter
try {
  matter = require('gray-matter')
} catch {
  matter = require(path.resolve(__dirname, '../../../node_modules/gray-matter'))
}

/** Average words-per-minute for technical reading */
const WPM = 200

/**
 * Strip MDX/JSX tags, import lines, frontmatter artefacts, and HTML comments
 * from content so word count is representative of prose.
 */
function extractProse(content) {
  return content
    .replace(/^---[\s\S]*?---/, '') // frontmatter (just in case)
    .replace(/^import\s.+$/gm, '') // import lines
    .replace(/export\s+(const|function|default)[^}]*\{[\s\S]*?\n\}/gm, '') // export blocks
    .replace(/<[^>]+>/g, ' ') // JSX/HTML tags
    .replace(/\{[^}]+\}/g, ' ') // JSX expressions
    .replace(/```[\s\S]*?```/g, ' CODE ') // code blocks → single word
    .replace(/`[^`]+`/g, ' ') // inline code
    .replace(/\s+/g, ' ')
    .trim()
}

function estimateReadTime(content) {
  const prose = extractProse(content)
  const words = prose.split(/\s+/).filter(Boolean).length
  const minutes = Math.ceil(words / WPM)
  return `${Math.max(1, minutes)} min`
}

function getTutorialFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const filePath = path.join(dir, entry.name)
    if (entry.isDirectory()) return getTutorialFiles(filePath)
    return /\.(md|mdx)$/.test(entry.name) ? [filePath] : []
  })
}

/**
 * @param {string} siteDir
 * @returns {Array<{id: string, title: string, description: string, href: string, tags: string[], time: string}>}
 */
function loadTutorials(siteDir) {
  const tutorialsDir = path.join(siteDir, 'docs', 'learn', 'tutorials')
  const tutorialFiles = getTutorialFiles(tutorialsDir)

  const tutorials = []

  for (const filePath of tutorialFiles) {
    if (path.basename(filePath) === 'index.mdx') continue

    const raw = fs.readFileSync(filePath, 'utf8')
    const { data: fm, content } = matter(raw)

    const slug = path.basename(filePath).replace(/\.(md|mdx)$/, '')

    tutorials.push({
      id: slug,
      title: fm.title || slug,
      description: fm.description || '',
      href: fm.slug || `/tutorials/${slug}`,
      tags: Array.isArray(fm.tags) ? fm.tags : [],
      time: estimateReadTime(content),
      sidebar_position: fm.sidebar_position ?? 999
    })
  }

  // Sort by sidebar_position so order matches the sidebar
  tutorials.sort((a, b) => a.sidebar_position - b.sidebar_position)

  return tutorials
}

/** @type {import('@docusaurus/types').PluginModule} */
module.exports = function tutorialsDataPlugin(context) {
  return {
    name: 'tutorials-data',
    async loadContent() {
      return loadTutorials(context.siteDir)
    },
    async contentLoaded({ content, actions }) {
      const { setGlobalData } = actions
      setGlobalData(content)
    }
  }
}
