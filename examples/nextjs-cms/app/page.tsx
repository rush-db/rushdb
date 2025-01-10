'use client'

import {
  headingsPlugin,
  listsPlugin,
  markdownShortcutPlugin,
  MDXEditor,
  quotePlugin,
  thematicBreakPlugin
} from '@mdxeditor/editor'
import '@mdxeditor/editor/style.css'

export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-between">
      <MDXEditor
        markdown={'# Hello World'}
        plugins={[
          headingsPlugin(),
          listsPlugin(),
          quotePlugin(),
          thematicBreakPlugin(),
          markdownShortcutPlugin()
        ]}
      />
      ;
    </main>
  )
}
