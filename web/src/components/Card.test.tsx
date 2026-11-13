import { render, screen } from "@testing-library/react"
import { Card } from "./Card"

test("renders its children and an optional title", () => {
  render(
    <Card title="Connections">
      <p>card body</p>
    </Card>,
  )
  expect(screen.getByRole("heading", { name: "Connections" })).toBeInTheDocument()
  expect(screen.getByText("card body")).toBeInTheDocument()
})
