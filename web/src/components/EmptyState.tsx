import type { ReactNode } from "react"

interface EmptyStateProps {
  title: string
  hint?: string
  action?: ReactNode
}

export function EmptyState({ title, hint, action }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <h3 className="card-title">{title}</h3>
      {hint ? <p className="muted small">{hint}</p> : null}
      {action ?? null}
    </div>
  )
}
