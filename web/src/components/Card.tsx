import type { ReactNode } from "react"

interface CardProps {
  title?: string
  className?: string
  children: ReactNode
}

export function Card({ title, className, children }: CardProps) {
  return (
    <section className={className ? `card ${className}` : "card"}>
      {title ? <h2 className="card-title">{title}</h2> : null}
      {children}
    </section>
  )
}
