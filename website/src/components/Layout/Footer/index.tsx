import { Github, Linkedin } from 'lucide-react'
import { Section } from '~/components/Section'
import Link from 'next/link'
import { socials } from '~/config/urls'
import { IconX } from '~/components/Layout/IconX'
import { IconDiscord } from '~/components/Layout/IconDiscord'
import { Logo } from '~/components/Logo'
import { GitHub } from '~/components/Icons/GitHub'

export function Footer() {
  return (
    <footer className="relative z-10 grid min-h-[30vh]">
      <div className="container py-16">
        <div className="grid grid-cols-2 items-start gap-4">
          <div className="justify-self-start">
            <Link href="/#hero" rel="noopener noreferrer" aria-label="Home page">
              <div className="flex items-center gap-4">
                <Logo className="text-content" width={60} height={60} />
                <span className="text-content typography-xl text-left font-bold">RushDB</span>
              </div>
            </Link>
          </div>

          <div className="flex flex-col justify-end gap-4">
            <a
              href={socials.emailUrl}
              className="text-content typography-xl text-right font-medium leading-none"
            >
              {socials.email}
            </a>
            <div className="flex justify-end gap-4">
              <Link
                href={socials.x}
                target="__blank"
                rel="noopener noreferrer"
                aria-label="X (Formerly Twitter)"
              >
                <IconX className="text-content" />
              </Link>
              <Link href={socials.github} target="__blank" rel="noopener noreferrer" aria-label="Github">
                <GitHub className="text-content h-6 w-6" />
              </Link>
              <Link href={socials.linkedIn} target="__blank" rel="noopener noreferrer" aria-label="LinkedIn">
                <Linkedin className="text-content" />
              </Link>
              {/*<Link href={socials.discord} target="__blank" rel="noopener noreferrer" aria-label="Discord">*/}
              {/*  <IconDiscord className="text-content h-12 w-12" />*/}
              {/*</Link>*/}
            </div>
          </div>
        </div>
        <div className="mt-16 grid gap-8 text-center">
          <div className="flex items-center justify-center gap-x-4 text-center">
            <Link
              href="/privacy-policy"
              target="__blank"
              rel="noopener noreferrer"
              aria-label="Privacy Policy"
            >
              Privacy Policy
            </Link>
            <Link
              href="/terms-of-service"
              target="__blank"
              rel="noopener noreferrer"
              aria-label="Terms of Service"
            >
              Terms of Service
            </Link>
            <Link
              href="/cookie-policy"
              target="__blank"
              rel="noopener noreferrer"
              aria-label="Terms of Service"
            >
              Cookie Policy
            </Link>
          </div>
          <p className="text-content2 text-sm font-medium leading-snug tracking-tight md:text-sm">
            © {new Date().getFullYear()}, Collect Software Inc.
          </p>
        </div>
      </div>
    </footer>
  )
}
