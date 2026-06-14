import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ColorModeButton, ColorModeProvider } from "./ColorMode";

beforeEach(() => {
  window.localStorage.clear();
});

describe("ColorMode", () => {
  it("表示モードを切り替えて保存する", () => {
    render(
      <ColorModeProvider>
        <ColorModeButton />
      </ColorModeProvider>,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "ダークモードに切り替え" }),
    );

    expect(
      screen.getByRole("button", { name: "ライトモードに切り替え" }),
    ).toBeInTheDocument();
    expect(window.localStorage.getItem("saas-platform:color-mode:v1")).toBe(
      "dark",
    );
  });
});
