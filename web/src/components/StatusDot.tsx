export type StatusTone = "neutral" | "good" | "warn" | "bad"

interface StatusDotProps {
  label: string
  tone?: StatusTone
}

const TONE_CLASS: Record<StatusTone, string> = {
  neutral: "bg-muted",
  good: "bg-good",
  warn: "bg-warn",
  bad: "bg-bad",
}

/** A status indicator that always includes a readable text label. */
export function StatusDot({ label, tone = "neutral" }: StatusDotProps) {
  return (
    <span className="inline-flex items-center gap-2 text-sm text-ink">
      <span aria-hidden="true" className={`size-2 rounded-full ${TONE_CLASS[tone]}`} />
      <span>{label}</span>
    </span>
  )
}
