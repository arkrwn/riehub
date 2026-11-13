import type { ReactNode } from "react"

interface RailProps {
  top: ReactNode
  bottom: ReactNode
}

export function Rail({ top, bottom }: RailProps) {
  return (
    <aside className="rail" aria-label="Sidebar">
      <div className="rail-pill">{top}</div>
      <div className="rail-pill rail-bottom">{bottom}</div>
    </aside>
  )
}
