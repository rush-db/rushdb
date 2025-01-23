import { FC, useState } from 'react'
import { copyToClipboard } from '~/utils/copyToClipboard'
import { Button } from '~/components/Button'
import { CopyCheck, CopyIcon } from 'lucide-react'

export const CopyButton: FC<{ text: string }> = ({ text }) => {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    setCopied(true)
    copyToClipboard(text, {
      callback: () => {
        setTimeout(() => {
          setCopied(false)
        }, 800)
      }
    })
  }
  return (
    <Button variant={'custom'} onClick={copy} size={'small'} className="">
      {copied ?
        <CopyCheck />
      : <CopyIcon />}
    </Button>
  )
}
