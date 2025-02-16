import { MainCta } from '~/components/Button'

export const CallToAction = ({
  text = 'Ship Features, Not Infrastructure - Try RushDB Cloud Today',
  buttonText = 'Get Started Now',
  description = "Leverage RushDB's graph-powered, zero-config database to build modern apps faster. Push data, query intelligently, and focus on features while RushDB handles the complexity."
}: {
  text?: string
  description?: string
  buttonText?: string
}) => (
  <div className="bg-secondary xs:rounded-none container my-16 flex flex-row items-center justify-between rounded-xl border p-6 sm:flex-col sm:items-stretch sm:p-5 md:col-span-12">
    <div className="flex flex-col gap-5">
      <p className="typography-xl">{text}</p>

      {description && <p className="typography-base max-w-2xl !font-normal">{description}</p>}
      <MainCta variant="accent" className={'shrink-1 w-fit'} size={'small'}>
        {buttonText}
      </MainCta>
    </div>
  </div>
)
