import { fireEvent, render, screen } from "@testing-library/react"
import { NavPills } from "./NavPills"

const items = [
  { id: "overview", label: "Overview" },
  { id: "chat", label: "Chat" },
]

test("marks the active item and reports selections", () => {
  const onSelect = vi.fn()
  render(<NavPills items={items} active="chat" onSelect={onSelect} />)
  const chat = screen.getByRole("button", { name: "Chat" })
  expect(chat).toHaveClass("active")
  expect(chat).toHaveAttribute("aria-current", "page")
  expect(screen.getByRole("button", { name: "Overview" })).not.toHaveClass("active")
  fireEvent.click(screen.getByRole("button", { name: "Overview" }))
  expect(onSelect).toHaveBeenCalledWith("overview")
})
