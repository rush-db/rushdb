import { Portal } from '@radix-ui/react-portal'
import classNames from 'classnames'
import { motion, AnimatePresence } from 'framer-motion'
import { ArrowUpRight, Github, Menu, X } from 'lucide-react'
import Link from 'next/link'
import { ComponentPropsWithoutRef, useCallback, useEffect, useState } from 'react'
import { Button, MainCta } from '~/components/Button'
import { IconButton } from '~/components/IconButton'
import { IconDiscord } from '~/components/Layout/IconDiscord'
import { IconX } from '~/components/Layout/IconX'
import { Logo } from '~/components/Logo'
import { links, socials } from '~/config/urls'

export function MenuItem({
  className,
  children,

  ...props
}: Omit<ComponentPropsWithoutRef<typeof Button<typeof Link>>, 'variant'>) {
  return (
    <Button
      as={Link}
      className={classNames('flex justify-between', className)}
      variant="primaryText"
      {...props}
    >
      {children}

      <ArrowUpRight />
    </Button>
  )
}

function MobileMenu() {
  const [open, setOpen] = useState(false)

  const handleScroll = useCallback(() => {
    setOpen(false)
  }, [])

  useEffect(() => {
    return window.removeEventListener('scroll', handleScroll)
  }, [handleScroll])

  return (
    <>
      <IconButton
        variant={open ? 'outline' : 'primaryText'}
        className="z-10 ml-3 hidden sm:grid"
        aria-label="Menu"
        onClick={() =>
          setOpen((current) => {
            const next = !current
            if (!next) {
              window.addEventListener('scroll', handleScroll)
            }
            return next
          })
        }
      >
        {open ?
          <X />
        : <Menu />}
      </IconButton>

      <AnimatePresence>
        {open && (
          <Portal asChild>
            <motion.div
              initial={{ y: '-100%' }}
              animate={{ y: 0 }}
              exit={{ y: '-100%' }}
              transition={{
                type: 'tween',
                duration: 0.15,
                easing: 'easeOut'
              }}
              className="bg-fill/90 fixed top-0 z-20 w-full rounded-b-2xl pb-4 pt-24 shadow-lg backdrop-blur-sm"
            >
              <div className="container divide-y">
                <MenuItem href={links.pricing}>Pricing</MenuItem>
                <MenuItem href={socials.blog}>Blog</MenuItem>
                <MenuItem href={links.docs}>Docs</MenuItem>
                {/*<MenuItem href={links.tutorials}>Tutorials</MenuItem>*/}
              </div>

              <div className="bg-stroke my-4 h-0.5" />

              <div className="container divide-y">
                <h6 className="typography-base text-content2 px-3">Socials</h6>

                <MenuItem as={Link} href={socials.discord} target="_blank" rel="noreferrer noopener">
                  <div className="flex items-center gap-2">
                    <IconDiscord height={18} width={18} />
                    Discord
                  </div>
                </MenuItem>
                <MenuItem as={Link} href={socials.x} target="_blank" rel="noreferrer noopener">
                  <div className="flex items-center gap-2">
                    <IconX height={18} width={18} />X (Twitter)
                  </div>
                </MenuItem>
                <MenuItem as={Link} href={socials.github} target="_blank" rel="noreferrer noopener">
                  <div className="flex items-center gap-2">
                    <Github height={18} width={18} />
                    Github
                  </div>
                </MenuItem>
              </div>
            </motion.div>
          </Portal>
        )}
      </AnimatePresence>
    </>
  )
}

function Nav() {
  return (
    <nav className="bg-fill/40 border-stroke-dark flex items-center justify-self-center rounded-full border p-2 shadow-2xl backdrop-blur-sm sm:justify-self-end">
      <div className="mr-3 flex items-center gap-2 sm:hidden">
        <Button variant="primaryText" as={Link} size="small" href={links.docs} className="!rounded-full">
          Docs
        </Button>
        <Button variant="primaryText" as={Link} size="small" href={links.docs} className="!rounded-full">
          Use Cases
        </Button>
        <Button variant="primaryText" as={Link} size="small" href={socials.blog} className="!rounded-full">
          Blog
        </Button>
        <Button variant="primaryText" as={Link} size="small" href={links.pricing} className="!rounded-full">
          Pricing
        </Button>
      </div>

      <div
        key={'nav-socials'}
        className="flex min-w-0 items-center gap-4 overflow-hidden whitespace-nowrap sm:!hidden"
      >
        <IconButton
          variant="custom"
          size="small"
          as={Link}
          href={socials.github}
          target="_blank"
          rel="noreferrer noopener"
          title="Github"
        >
          <Github />
        </IconButton>
      </div>

      <MobileMenu />
    </nav>
  )
}

export const Header = () => {
  return (
    <header
      className={
        'fixed top-0 z-30 flex h-[100px] w-full flex-row items-center justify-between bg-transparent'
      }
    >
      <div className="container grid grid-cols-3 items-center justify-between sm:grid-cols-2">
        <Link href="/">
          <Logo className="h-[60px] w-[60px]" />
        </Link>

        <Nav />
        <MainCta variant="accent" size="medium" className="ml-4 justify-self-end sm:hidden" />
      </div>
    </header>
  )
}
