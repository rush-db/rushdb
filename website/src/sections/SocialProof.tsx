import { Check, Star } from 'lucide-react'
import Link from 'next/link'
import { socials } from '~/config/urls'
import { GitHub } from '~/components/Icons/GitHub'

export const SocialProof = () => {
  return (
    <section className="container">
      <div className="outline-stroke rounded-b-[50px] py-12 text-center outline outline-1 outline-offset-0">
        <div className="text-md flex items-center justify-center gap-8 md:flex-col md:gap-6">
          <div className="flex items-center gap-2">
            <GitHub className="h-5 w-5" />
            <span className="font-medium">Open Source</span>
          </div>

          <div className="flex items-center gap-2">
            <Star className="fill-accent text-accent h-5 w-5" />
            <span className="font-medium">Production Ready</span>
          </div>

          <div className="flex items-center gap-2">
            <span className="flex h-5 w-5 items-center justify-center rounded-full bg-green-500 text-sm font-bold text-white">
              <Check size={16} />
            </span>
            <span className="font-medium">Zero Config</span>
          </div>

          <div className="flex items-center gap-2">
            <span className="bg-accent flex h-5 w-5 items-center justify-center rounded-full text-sm font-bold text-white">
              2
            </span>
            <span className="font-medium">Free Projects Forever</span>
          </div>
        </div>

        <div className="text-content3 text-md mt-8">
          <p>Trusted by developers who ship fast</p> {/* <span className="mx-2">•</span> */}
          <Link href={socials.github} target="_blank" className="hover:text-accent underline">
            Star us on GitHub
          </Link>
        </div>
      </div>
    </section>
  )
}
