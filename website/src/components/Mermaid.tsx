import type { MermaidProps } from 'mdx-mermaid/lib/Mermaid'
import dynamic from 'next/dynamic'
import type { Config } from 'mdx-mermaid/lib/config.model'
import { useTheme } from '~/contexts/ThemeContext'
import { useEffect, useState } from 'react'

const MdxMermaid = dynamic(() => import('mdx-mermaid/lib/Mermaid').then((res) => res.Mermaid), { ssr: false })

export const Mermaid: React.FC<MermaidProps> = ({ ...props }) => {
  const { theme } = useTheme()
  const [mermaidLoaded, setMermaidLoaded] = useState(false)

  useEffect(() => {
    setTimeout(() => {
      // This is a workaround to ensure that Mermaid is loaded after the initial render
      setMermaidLoaded(true)
    }, 250)
  }, [])

  const config: Config = {
    mermaid: {
      fontFamily: 'inherit',
      theme: theme === 'dark' ? 'dark' : 'default'
    }
  }

  return <MdxMermaid {...props} config={config} />
}
