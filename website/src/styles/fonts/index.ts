import localFont from "next/font/local"
import { Manrope, DM_Serif_Display } from "next/font/google"

export const kyivTypeSans = localFont({
  src: [
    {
      path: "./kyiv-sans/KyivTypeSans-Bold.woff2",
      weight: "600",
      style: "bold",
    },
    // {
    //   path: "./kyiv-sans/KyivTypeSans-Medium.woff2",
    //   weight: "500",
    //   style: "medium",
    // },
  ],
  variable: "--font-kyiv-sans",
})

export const dmSerifDisplay = DM_Serif_Display({
  subsets: ["latin"],
  variable: "--font-dm-serif-display",
  style: ["italic"],
  weight: "400",
})

export const jetBrainsMono = localFont({
  src: [
    {
      path: "./jet-brains-mono/JetBrainsMono-Regular.woff2",
      weight: "400",
      style: "regular",
    },
    // {
    //   path: "./jet-brains-mono/JetBrainsMono-Medium.woff2",
    //   weight: "500",
    //   style: "medium",
    // },
  ],
  variable: "--font-jet-brains-mono",
})

export const manrope = Manrope({
  subsets: ["latin"],
  variable: "--font-manrope",
  style: ["normal"],
})
