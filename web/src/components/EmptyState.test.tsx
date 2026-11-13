import { render, screen } from "@testing-library/react"
import { EmptyState } from "./EmptyState"

test("shows the title, optional hint and optional action", () => {
  render(<EmptyState title="No connections" hint="Add one to get started" />)
  expect(screen.getByText("No connections")).toBeInTheDocument()
  expect(screen.getByText("Add one to get started")).toBeInTheDocument()
})
