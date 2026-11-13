export type ToastTone = "good" | "warn" | "bad" | "info"

interface ToastProps {
  message: string
  tone?: ToastTone
}

export function Toast({ message, tone = "info" }: ToastProps) {
  return (
    <div className={`toast toast-${tone}`} role="status">
      <span className="toast-dot" aria-hidden="true" />
      <span>{message}</span>
    </div>
  )
}
