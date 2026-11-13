import { useState } from "react"
import { Button } from "./Button"
import { Dialog } from "./Dialog"

interface SecretRevealProps {
  open: boolean
  value: string
  title?: string
  onClose: () => void
}

/*
  Shows a secret once, with a copy button. The parent owns the value
  and drops it from state when the dialog closes, so the secret is
  never kept after that.
*/
export function SecretReveal({ open, value, title = "Secret", onClose }: SecretRevealProps) {
  const [copied, setCopied] = useState(false)

  async function copy() {
    await navigator.clipboard.writeText(value)
    setCopied(true)
  }

  function close() {
    setCopied(false)
    onClose()
  }

  return (
    <Dialog open={open} onClose={close} title={title}>
      <p className="muted small">Show this once. It is not kept after the dialog closes.</p>
      <code className="secret-value">{value}</code>
      <div className="dialog-actions">
        <Button variant="secondary" onClick={copy}>
          {copied ? "Copied" : "Copy"}
        </Button>
        <Button onClick={close}>Done</Button>
      </div>
    </Dialog>
  )
}
