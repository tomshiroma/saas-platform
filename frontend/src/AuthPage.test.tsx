import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthPage } from "./AuthPage";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("AuthPage password reset", () => {
  it("パスワード再設定メールを申請できる", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          message:
            "入力された情報に一致するアカウントがある場合、再設定メールを送信しました。",
        }),
        {
          status: 202,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    renderAuthPage();

    fireEvent.click(
      screen.getByRole("button", { name: "パスワードをお忘れの方" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "再設定メールを送信" }),
    );

    expect(
      await screen.findByText(
        "入力された情報に一致するアカウントがある場合、再設定メールを送信しました。",
      ),
    ).toBeInTheDocument();
  });

  it("メールリンクから新しいパスワードを設定できる", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(null, { status: 204 }),
    );

    renderAuthPage("/reset-password?token=valid-reset-token-with-enough-length");

    fireEvent.change(screen.getByLabelText("新しいパスワード"), {
      target: { value: "new-development-password" },
    });
    fireEvent.change(screen.getByLabelText("新しいパスワード（確認）"), {
      target: { value: "new-development-password" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "パスワードを変更" }),
    );

    expect(
      await screen.findByText(
        "パスワードを変更しました。新しいパスワードでログインしてください。",
      ),
    ).toBeInTheDocument();
  });
});

function renderAuthPage(initialEntry = "/") {
  render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <AuthPage onAuthenticated={() => undefined} />
    </MemoryRouter>,
  );
}
