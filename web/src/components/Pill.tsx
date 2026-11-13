import type { ReactNode } from "react"

export type PillTone = "neutral" | "good" | "warn" | "bad"

interface PillProps {
  tone?: PillTone
  children: ReactNode
}

export function Pill({ tone = "neutral", children }: PillProps) {
  return <span className={tone === "neutral" ? "pill" : `pill pill-${tone}`}>{children}</span>
}
