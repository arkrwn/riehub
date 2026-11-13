import type { ReactNode } from "react"

interface RailProps {
  top: ReactNode
  middle: ReactNode
  bottom: ReactNode
}

export function Rail({ top, middle, bottom }: RailProps) {
  return (
    <aside className="rail" aria-label="Sidebar">
      <div className="rail-pill">{top}</div>
      <div className="rail-pill rail-middle">{middle}</div>
      <div className="rail-pill rail-bottom">{bottom}</div>
    </aside>
  )
}
