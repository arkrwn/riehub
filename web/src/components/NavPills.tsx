export interface NavItem {
  id: string
  label: string
}

interface NavPillsProps {
  items: NavItem[]
  active: string
  onSelect?: (id: string) => void
}

export function NavPills({ items, active, onSelect }: NavPillsProps) {
  return (
    <nav className="nav-pills" aria-label="Primary">
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className={item.id === active ? "nav-pill active" : "nav-pill"}
          aria-current={item.id === active ? "page" : undefined}
          onClick={() => onSelect?.(item.id)}
        >
          {item.label}
        </button>
      ))}
    </nav>
  )
}
