import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import WelcomeScreen, { welcomeOnStartup } from "../components/WelcomeScreen";

describe("WelcomeScreen", () => {
  it("shows loading, then the library size, and closes with the button or Escape", () => {
    let closed = 0;
    const { rerender } = render(<WelcomeScreen total={null} error={null} onClose={() => closed++} />);
    expect(screen.getByText("Cargando biblioteca…")).toBeTruthy();
    rerender(<WelcomeScreen total={1160} error={null} onClose={() => closed++} />);
    expect(screen.getByText("1160 juegos en la biblioteca")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Empezar" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(closed).toBe(2);
  });

  it("remembers the 'Mostrar al iniciar' preference", () => {
    localStorage.setItem("rpglib.welcome", "on");
    expect(welcomeOnStartup()).toBe(true);
    render(<WelcomeScreen total={1} error={null} onClose={() => {}} />);
    const box = screen.getByRole("checkbox", { name: /Mostrar al iniciar/ }) as HTMLInputElement;
    expect(box.checked).toBe(true);
    fireEvent.click(box);
    expect(localStorage.getItem("rpglib.welcome")).toBe("off");
    expect(welcomeOnStartup()).toBe(false);
  });

  it("shows the load error instead of hiding it", () => {
    render(<WelcomeScreen total={null} error="DB no encontrada" onClose={() => {}} />);
    expect(screen.getByText(/No se pudo abrir la biblioteca: DB no encontrada/)).toBeTruthy();
  });
});
