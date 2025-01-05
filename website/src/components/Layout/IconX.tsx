import { ComponentPropsWithoutRef } from "react"

export function IconX({
  strokeWidth = "2",
  ...props
}: ComponentPropsWithoutRef<"svg">) {
  return (
    <svg
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <polygon points="2.07 2 7.04 2 21.93 22 16.96 22 2.07 2" />
      <line x1="10.4" x2="2.24" y1="13.19" y2="22" />
      <line x1="20.76" x2="13.15" y1="2" y2="10.21" />
    </svg>
  )
}
