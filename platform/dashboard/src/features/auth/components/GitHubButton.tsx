import { useState } from 'react'
import { BASE_URL } from '~/config'
import { Button } from '~/elements/Button'
import { DialogLoadingOverlay } from '~/elements/Dialog'
import { getRoutePath } from '~/lib/router'
import { Github } from 'lucide-react'

const GITHUB_URL = `${BASE_URL}/api/v1/auth/github?redirectUrl=${
  window.location.origin
}${getRoutePath('oauth')}`

export function GithubButton() {
  const [loading, setLoading] = useState(false)

  return (
    <>
      <Button
        as="a"
        href={GITHUB_URL}
        size="large"
        variant="primary"
        onClick={() => setLoading(true)}
        className="flex grow items-center justify-center"
      >
        <Github /> GitHub
      </Button>

      {loading && <DialogLoadingOverlay />}
    </>
  )
}
