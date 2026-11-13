import { render, screen } from "@testing-library/react"
import { Rail } from "./Rail"

test("renders the top and bottom slots inside labelled pills", () => {
  render(
    <Rail
      top={<button type="button">theme</button>}
      bottom={<button type="button">logout</button>}
    />,
  )
  const rail = screen.getByRole("complementary", { name: "Sidebar" })
  expect(rail).toBeInTheDocument()
  expect(screen.getByRole("button", { name: "theme" }).parentElement).toHaveClass("rail-pill")
  expect(screen.getByRole("button", { name: "logout" }).parentElement).toHaveClass(
    "rail-pill",
    "rail-bottom",
  )
})
