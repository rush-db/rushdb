import { useState } from "react"
import {
  Section,
  SectionHeader,
  SectionSubtitle,
  SectionTitle,
} from "~/components/Section"

import { ComponentPropsWithoutRef, ReactNode } from "react"
import cx from "classnames"
import { Button } from "~/components/Button"
import { ArrowUpRight, Check, LucideMail, MessageCircle } from "lucide-react"

import Link from "next/link"
import { links } from "~/config/urls"

function Feat({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <li className="py-3 text-start">
      <div>
        <span className="text-start text-base font-medium">
          <Check className="w-4 h-4 shrink-0 mr-3" />

          {title}
        </span>
      </div>

      {subtitle && (
        <div className="pl-7 text-content2 text-sm text-start">{subtitle}</div>
      )}
    </li>
  )
}

function PricingCard({
  title,
  description,
  action,
  className,
  price,
  children,
  featured,
  ...props
}: ComponentPropsWithoutRef<"div"> & {
  title?: ReactNode
  description: ReactNode
  action: ReactNode
  price?: number | "free"
  featured?: boolean
}) {
  return (
    <article
      className={cx(
        "flex flex-col items-center text-center bg-secondary rounded-xl first:rounded-l-3xl last:rounded-r-3xl [&:first-child>div]:rounded-l-[20px] [&:last-child>div]:rounded-r-[20px] shadow-lg p-1 sm:!rounded-xl",
        featured
          ? "bg-gradient-to-br from-accent-hover to-accent-orange"
          : "bg-secondary",
        className,
      )}
      {...props}
    >
      <div
        className={cx(
          "flex h-full w-full flex-col items-center rounded-lg p-5 sm:!rounded-lg",
          featured ? "bg-fill" : "bg-secondary",
        )}
      >
        <h3 className="font-bold typography-sm h-5">{title}</h3>

        <span className="font-bold typography-2xl uppercase">
          {price !== "free" && price !== undefined && "$"}
          {price ?? "Custom"}
        </span>

        <p className="mb-3 text-content2">{description}</p>

        <div className="grid w-full">{action}</div>
        {price !== "free" && price !== undefined ? (
          <p className="text-content3 mt-1 text-sm">No credit card required</p>
        ) : (
          <p className="mt-1 text-xs text-transparent select-none">-</p>
        )}

        <ul className="flex flex-col w-full mt-5 divide-y">{children}</ul>
      </div>
    </article>
  )
}

enum Variants {
  Cloud = "cloud",
  OnPremise = "on-premise",
}

export function Pricing() {
  const [variant, setVariant] = useState<Variants>(Variants.Cloud)

  return (
    <Section className="container">
      <SectionHeader className="text-center">
        <SectionTitle className="m-auto max-w-3xl">Pricing</SectionTitle>

        <SectionSubtitle>
          {variant === Variants.Cloud &&
            "Start building for free with the power to scale."}
        </SectionSubtitle>
      </SectionHeader>

      <div className="grid sm:grid-cols-1 md:grid-cols-2 grid-cols-3 gap-3">
        {variant === Variants.Cloud && (
          <>
            <PricingCard
              price="free"
              description="Forever"
              action={
                <Button
                  size="small"
                  variant="secondary"
                  as={Link}
                  href={links.app}
                  target="_blank"
                >
                  Start Building
                  <ArrowUpRight />
                </Button>
              }
            >
              <Feat title="2 Projects" />
              <Feat title="1 000 Records" />
              <Feat title="Unlimited API Requests" subtitle="Up to 10 RPS" />
              <Feat title="Community Support" />
            </PricingCard>
            <PricingCard
              price={11}
              featured
              title="Pro"
              description="Monthly"
              action={
                <Button
                  size="small"
                  variant="accent"
                  as={Link}
                  href={links.app}
                >
                  Start for Free
                  <ArrowUpRight />
                </Button>
              }
            >
              <Feat title="Unlimited Projects" />
              <Feat
                title="100 000 Records"
                // subtitle="then $1 per 10 000 Records"
                subtitle="No upper limits or charges in technical preview"
              />
              <Feat title="Unlimited API Requests" subtitle="No RPS limits" />
              <Feat title="Priority Support" />
            </PricingCard>
            <PricingCard
              title="Business"
              description="Contact sales"
              action={
                <Button
                  size="small"
                  variant="primary"
                  as={Link}
                  href={links.contactUs}
                >
                  Contact Us
                  <LucideMail />
                </Button>
              }
            >
              <Feat title="Unlimited Everything" />
              <Feat title="White Labeling & Customizations" />
              <Feat
                title="On-Premises Deployment"
                subtitle="Lifetime updates on demand"
              />
              <Feat title="Dedicated Support" />
            </PricingCard>
          </>
        )}
      </div>
    </Section>
  )
}
