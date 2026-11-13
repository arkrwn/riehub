import type { ButtonHTMLAttributes } from "react"

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger"

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
}

export function Button({ variant = "primary", className, type, ...rest }: ButtonProps) {
  const classes = ["btn", `btn-${variant}`, className].filter(Boolean).join(" ")
  return <button type={type ?? "button"} className={classes} {...rest} />
}
