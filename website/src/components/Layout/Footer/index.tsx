import { Github, Linkedin } from 'lucide-react'
import { Section } from '~/components/Section'
import Link from 'next/link'
import { socials } from '~/config/urls'
import { IconX } from '~/components/Layout/IconX'
import { IconDiscord } from '~/components/Layout/IconDiscord'
import { Logo } from '~/components/Logo'

export function Footer() {
  return (
    <footer className="relative z-10 grid min-h-[30vh]">
      <div className="container py-16 sm:py-0">
        <div className="grid grid-cols-2 items-start gap-4 sm:grid-cols-1 sm:text-center">
          <div className="justify-self-start sm:order-2 sm:mt-5 sm:justify-self-center">
            <div className="flex gap-10">
              <Logo className="text-content sm:m-auto sm:h-[60px] sm:w-[60px]" width={140} height={140} />
              <div className="m-auto flex flex-col gap-4">
                <h2 className="text-content justify-self-end text-left text-[80px] font-bold leading-[0.7] tracking-tight md:text-xl md:leading-6">
                  RushDB
                </h2>
                <p className="text-content2 justify-self-start text-sm font-medium leading-snug tracking-tight sm:text-sm">
                  © {new Date().getFullYear()}, Collect Software Inc.
                </p>
              </div>
            </div>
          </div>

          <div className="grid items-end gap-8 justify-self-end text-center sm:justify-self-center">
            <a href={socials.emailUrl} className="text-content text-xl font-medium leading-none">
              {socials.email}
            </a>
            {/*<a*/}
            {/*  href="tel:+1(302)4809110"*/}
            {/*  className="text-xl leading-none font-medium text-content"*/}
            {/*>*/}
            {/*  +1 (302) 480 9110*/}
            {/*</a>*/}
            <div className="flex justify-between">
              <Link
                href={socials.x}
                target="__blank"
                rel="noopener noreferrer"
                aria-label="X (Formerly Twitter)"
              >
                <IconX strokeWidth={1} className="text-content h-12 w-12" />
              </Link>
              <Link href={socials.github} target="__blank" rel="noopener noreferrer" aria-label="Github">
                <Github strokeWidth={1} className="text-content h-12 w-12" />
              </Link>
              <Link href={socials.linkedIn} target="__blank" rel="noopener noreferrer" aria-label="LinkedIn">
                <Linkedin strokeWidth={1} className="text-content h-12 w-12" />
              </Link>
              <Link href={socials.discord} target="__blank" rel="noopener noreferrer" aria-label="Discord">
                <IconDiscord className="text-content h-12 w-12" />
              </Link>
            </div>

            <div className="grid items-end gap-y-4 justify-self-end text-right md:mb-4 md:justify-self-center md:text-center">
              <p className="text-lg font-bold">Legal</p>
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
          </div>
        </div>
      </div>
    </footer>
  )
}
