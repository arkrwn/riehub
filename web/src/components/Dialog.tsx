import { useEffect, type ReactNode } from "react"

interface DialogProps {
  open: boolean
  onClose: () => void
  title?: string
  children?: ReactNode
}

export function Dialog({ open, onClose, title, children }: DialogProps) {
  useEffect(() => {
    if (!open) return
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("keydown", onKey)
    return () => document.removeEventListener("keydown", onKey)
  }, [open, onClose])

  if (!open) return null

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="dialog" role="dialog" aria-modal="true" aria-label={title}>
        {title ? <h2 className="card-title">{title}</h2> : null}
        {children}
      </div>
    </div>
  )
}
