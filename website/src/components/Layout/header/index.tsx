import { Portal } from "@radix-ui/react-portal"
import classNames from "classnames"
import {
  useScroll,
  motion,
  useMotionValueEvent,
  AnimatePresence,
} from "framer-motion"
import { ArrowUpRight, Github, Menu, X } from "lucide-react"
import Link from "next/link"
import {
  ComponentPropsWithoutRef,
  useCallback,
  useEffect,
  useState,
} from "react"
import { Button, MainCta } from "~/components/Button"
import { IconButton } from "~/components/IconButton"
import { IconDiscord } from "~/components/Layout/IconDiscord"
import { IconX } from "~/components/Layout/IconX"
import { Logo } from "~/components/Logo"
import { links, socials } from "~/config/urls"

export function MenuItem({
  className,
  children,

  ...props
}: Omit<ComponentPropsWithoutRef<typeof Button<typeof Link>>, "variant">) {
  return (
    <Button
      as={Link}
      className={classNames("flex justify-between", className)}
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
    return window.removeEventListener("scroll", handleScroll)
  }, [handleScroll])

  return (
    <>
      <IconButton
        variant={open ? "outline" : "primaryText"}
        className="hidden sm:grid ml-3 z-10"
        aria-label="Menu"
        onClick={() =>
          setOpen((current) => {
            const next = !current
            if (!next) {
              window.addEventListener("scroll", handleScroll)
            }
            return next
          })
        }
      >
        {open ? <X /> : <Menu />}
      </IconButton>

      <AnimatePresence>
        {open && (
          <Portal asChild>
            <motion.div
              initial={{ y: "-100%" }}
              animate={{ y: 0 }}
              exit={{ y: "-100%" }}
              transition={{
                type: "tween",
                duration: 0.15,
                easing: "easeOut",
              }}
              className="fixed z-20 top-0 bg-fill/90 w-full pt-24 pb-4 rounded-b-2xl shadow-lg backdrop-blur-sm"
            >
              <div className="container divide-y">
                <MenuItem href={links.pricing}>Pricing</MenuItem>
                <MenuItem href={socials.blog}>Blog</MenuItem>
                <MenuItem href={links.docs}>Docs</MenuItem>
                {/*<MenuItem href={links.tutorials}>Tutorials</MenuItem>*/}
              </div>

              <div className="h-0.5 bg-stroke my-4" />

              <div className="container divide-y">
                <h6 className="typography-base px-3 text-content2">Socials</h6>

                <MenuItem
                  as={Link}
                  href={socials.discord}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  <div className="flex gap-2 items-center">
                    <IconDiscord height={18} width={18} />
                    Discord
                  </div>
                </MenuItem>
                <MenuItem
                  as={Link}
                  href={socials.x}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  <div className="flex gap-2 items-center">
                    <IconX height={18} width={18} />X (Twitter)
                  </div>
                </MenuItem>
                <MenuItem
                  as={Link}
                  href={socials.github}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  <div className="flex gap-2 items-center">
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
  const [hasScrolled, setHasScrolled] = useState(false)
  const { scrollYProgress } = useScroll()

  useMotionValueEvent(scrollYProgress, "change", (latest) => {
    if (latest > 0.1) {
      setHasScrolled(true)
    } else {
      setHasScrolled(false)
    }
  })

  return (
    <nav className="flex items-center">
      <div className="flex items-center mr-3 sm:hidden">
        <Button
          variant="primaryText"
          as={Link}
          size="small"
          href={links.pricing}
        >
          Pricing
        </Button>
        <Button variant="primaryText" as={Link} size="small" href={links.docs}>
          Docs
        </Button>
        {/*<Button*/}
        {/*  variant="primaryText"*/}
        {/*  as={Link}*/}
        {/*  size="small"*/}
        {/*  href={links.tutorials}*/}
        {/*>*/}
        {/*  Tutorials*/}
        {/*</Button>*/}
        <Button
          variant="primaryText"
          as={Link}
          size="small"
          href={socials.blog}
        >
          Blog
        </Button>
      </div>

      <AnimatePresence>
        {!hasScrolled && (
          <motion.div
            key={"nav-socials"}
            initial={{ width: 0 }}
            animate={{ width: "auto" }}
            exit={{ width: 0 }}
            className="flex items-center gap-4 overflow-hidden whitespace-nowrap min-w-0 sm:!hidden"
          >
            <IconButton
              variant="secondary"
              size="small"
              as={Link}
              href={socials.discord}
              target="_blank"
              rel="noreferrer noopener"
              title="Discord"
            >
              <IconDiscord />
            </IconButton>
            <IconButton
              variant="secondary"
              size="small"
              as={Link}
              href={socials.x}
              target="_blank"
              rel="noreferrer noopener"
              title="X (Formerly Twitter)"
            >
              <IconX />
            </IconButton>
            <IconButton
              variant="secondary"
              size="small"
              as={Link}
              href={socials.github}
              target="_blank"
              rel="noreferrer noopener"
              title="Github"
            >
              <Github />
            </IconButton>
          </motion.div>
        )}
      </AnimatePresence>
      <MainCta variant="accent" size="small" className="ml-4" />

      <MobileMenu />
    </nav>
  )
}

export const Header = () => {
  return (
    <header
      className={
        "flex flex-row justify-between items-center fixed z-30 w-full top-0 bg-transparent h-[100px]"
      }
    >
      <div className="container flex flex-row justify-between">
        <Link href="/">
          <Logo className="h-[80px] w-[80px]" />
        </Link>

        <Nav />
      </div>
    </header>
  )
}
