import { fireEvent, render, screen, within } from "@testing-library/react"
import { DataTable, type DataTableColumn } from "./DataTable"

interface UserRow {
  id: string
  email: string
}

const columns: DataTableColumn<UserRow>[] = [
  { key: "email", header: "Email", render: (row) => row.email },
]

test("renders an accessible table and empty state", () => {
  render(
    <DataTable rows={[]} columns={columns} rowKey={(row) => row.id} caption="Users" emptyTitle="No users" />,
  )
  expect(screen.getByText("No users")).toBeInTheDocument()
})

test("selects visible rows and paginates", () => {
  const onSelectionChange = vi.fn()
  const rows = [
    { id: "1", email: "one@example.test" },
    { id: "2", email: "two@example.test" },
    { id: "3", email: "three@example.test" },
  ]
  render(
    <DataTable
      rows={rows}
      columns={columns}
      rowKey={(row) => row.id}
      caption="Users"
      pageSize={2}
      selectable
      onSelectionChange={onSelectionChange}
    />,
  )

  expect(screen.getByRole("table", { name: "Users" })).toBeInTheDocument()
  fireEvent.click(screen.getByRole("checkbox", { name: "Select all rows on this page" }))
  expect(onSelectionChange).toHaveBeenLastCalledWith(["1", "2"])
  expect(screen.queryByText("three@example.test")).not.toBeInTheDocument()
  fireEvent.click(screen.getByRole("button", { name: "Next" }))
  const table = screen.getByRole("table", { name: "Users" })
  expect(within(table).getByText("three@example.test")).toBeInTheDocument()
  expect(screen.getByText("3–3 of 3")).toBeInTheDocument()
})
