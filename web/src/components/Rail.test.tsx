import { render, screen } from "@testing-library/react"
import { Rail } from "./Rail"

test("renders keyboard-accessible top, middle and bottom slots", () => {
  render(
    <Rail
      top={<button type="button">theme</button>}
      middle={<button type="button">overview</button>}
      bottom={<button type="button">logout</button>}
    />,
  )
  const rail = screen.getByRole("complementary", { name: "Sidebar" })
  expect(rail).toBeInTheDocument()
  const theme = screen.getByRole("button", { name: "theme" })
  expect(theme.parentElement).toHaveClass("rail-pill")
  theme.focus()
  expect(theme).toHaveFocus()
  expect(screen.getByRole("button", { name: "overview" }).parentElement).toHaveClass(
    "rail-pill",
    "rail-middle",
  )
  expect(screen.getByRole("button", { name: "logout" }).parentElement).toHaveClass(
    "rail-pill",
    "rail-bottom",
  )
})
