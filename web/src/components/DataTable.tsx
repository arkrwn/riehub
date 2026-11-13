import { useMemo, useState, type ReactNode } from "react"
import { EmptyState } from "./EmptyState"

export interface DataTableColumn<T> {
  key: string
  header: string
  render: (row: T) => ReactNode
  headerClassName?: string
  cellClassName?: string
}

interface DataTableProps<T> {
  rows: T[]
  columns: DataTableColumn<T>[]
  rowKey: (row: T) => string
  pageSize?: number
  caption: string
  emptyTitle?: string
  emptyHint?: string
  selectable?: boolean
  onSelectionChange?: (selectedIds: string[]) => void
}

/** Accessible table with optional row selection and compact pagination. */
export function DataTable<T>({
  rows,
  columns,
  rowKey,
  pageSize = 10,
  caption,
  emptyTitle = "Nothing here yet",
  emptyHint,
  selectable = false,
  onSelectionChange,
}: DataTableProps<T>) {
  const [page, setPage] = useState(0)
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const safePageSize = Math.max(1, pageSize)
  const pageCount = Math.max(1, Math.ceil(rows.length / safePageSize))
  const currentPage = Math.min(page, pageCount - 1)
  const visibleRows = useMemo(
    () => rows.slice(currentPage * safePageSize, (currentPage + 1) * safePageSize),
    [currentPage, rows, safePageSize],
  )
  const visibleIds = visibleRows.map(rowKey)
  const allVisibleSelected = visibleIds.length > 0 && visibleIds.every((id) => selected.has(id))

  function updateSelection(next: Set<string>) {
    setSelected(next)
    onSelectionChange?.([...next])
  }

  function toggleAll() {
    const next = new Set(selected)
    if (allVisibleSelected) visibleIds.forEach((id) => next.delete(id))
    else visibleIds.forEach((id) => next.add(id))
    updateSelection(next)
  }

  function toggleRow(id: string) {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    updateSelection(next)
  }

  if (rows.length === 0) {
    return <EmptyState title={emptyTitle} hint={emptyHint} />
  }

  return (
    <div className="w-full overflow-x-auto">
      <table className="w-full border-collapse text-left text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead className="border-b border-line text-muted">
          <tr>
            {selectable ? (
              <th scope="col" className="w-10 px-3 py-3">
                <input
                  aria-label="Select all rows on this page"
                  type="checkbox"
                  checked={allVisibleSelected}
                  onChange={toggleAll}
                />
              </th>
            ) : null}
            {columns.map((column) => (
              <th
                key={column.key}
                scope="col"
                className={`px-3 py-3 font-medium ${column.headerClassName ?? ""}`}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {visibleRows.map((row) => {
            const id = rowKey(row)
            return (
              <tr key={id} className="border-b border-line last:border-0">
                {selectable ? (
                  <td className="px-3 py-3">
                    <input
                      aria-label={`Select row ${id}`}
                      type="checkbox"
                      checked={selected.has(id)}
                      onChange={() => toggleRow(id)}
                    />
                  </td>
                ) : null}
                {columns.map((column) => (
                  <td key={column.key} className={`px-3 py-3 ${column.cellClassName ?? ""}`}>
                    {column.render(row)}
                  </td>
                ))}
              </tr>
            )
          })}
        </tbody>
      </table>
      {pageCount > 1 ? (
        <nav aria-label={`${caption} pagination`} className="flex items-center justify-end gap-3 py-3">
          <span className="text-sm text-muted">
            {currentPage * safePageSize + 1}–{Math.min((currentPage + 1) * safePageSize, rows.length)} of {rows.length}
          </span>
          <button
            type="button"
            className="rounded-control-token border border-line px-3 py-2 text-ink disabled:opacity-50"
            onClick={() => setPage(currentPage - 1)}
            disabled={currentPage === 0}
          >
            Previous
          </button>
          <button
            type="button"
            className="rounded-control-token border border-line px-3 py-2 text-ink disabled:opacity-50"
            onClick={() => setPage(currentPage + 1)}
            disabled={currentPage >= pageCount - 1}
          >
            Next
          </button>
        </nav>
      ) : null}
    </div>
  )
}
