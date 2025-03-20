import Link from 'next/link'
import { HomeIcon } from 'lucide-react'

import { Button } from '~/components/Button'
import { links } from '~/config/urls'

export default function NotFoundPage() {
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center text-center">
      <h1 className="typography-4xl !font-extrabold sm:text-2xl">404 - Page Not Found</h1>
      <p className="text-content3 text-md mt-4 !font-medium md:text-base">
        Oops! The page you are looking for does not exist.
      </p>
      <div className="mt-8 flex gap-4">
        <Button as={Link} href={'/'} variant="accent" size="small">
          Homepage <HomeIcon className="ml-2 h-5 w-5" />
        </Button>
        <Button as={Link} href={links.getStarted} variant="outline" size="small">
          Docs
        </Button>
      </div>
    </div>
  )
}
