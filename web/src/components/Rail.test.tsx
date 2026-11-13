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
  const theme = screen.getByRole("button", { name: "theme" })
  expect(theme.parentElement).toHaveClass("rail-pill")
  theme.focus()
  expect(theme).toHaveFocus()
  expect(screen.getByRole("button", { name: "logout" }).parentElement).toHaveClass(
    "rail-pill",
    "rail-bottom",
  )
})
