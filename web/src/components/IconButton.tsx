import type { ButtonHTMLAttributes, ReactNode } from "react"

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string
  children: ReactNode
}

export function IconButton({ label, children, className, type, ...rest }: IconButtonProps) {
  const classes = ["icon-btn", className].filter(Boolean).join(" ")
  return (
    <button type={type ?? "button"} className={classes} aria-label={label} title={label} {...rest}>
      {children}
    </button>
  )
}
