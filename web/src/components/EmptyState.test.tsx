import { render, screen } from "@testing-library/react"
import { EmptyState } from "./EmptyState"

test("shows the title, hint and keyboard-accessible action", () => {
  render(
    <EmptyState
      title="No connections"
      hint="Add one to get started"
      action={<button type="button">Add connection</button>}
    />,
  )
  expect(screen.getByText("No connections")).toBeInTheDocument()
  expect(screen.getByText("Add one to get started")).toBeInTheDocument()
  const action = screen.getByRole("button", { name: "Add connection" })
  action.focus()
  expect(action).toHaveFocus()
})
