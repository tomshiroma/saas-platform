import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthPage } from "./AuthPage";

vi.mock("qrcode", () => ({
  default: {
    toDataURL: vi.fn().mockResolvedValue("data:image/png;base64,test-qr"),
  },
}));

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

describe("AuthPage tenant administrator MFA", () => {
  it("初回ログインでTOTPを設定しリカバリーコードを表示する", async () => {
    vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        jsonResponse({
          status: "mfa_setup_required",
          challenge_token: "tenant.challenge",
          secret: "JBSWY3DPEHPK3PXP",
          provisioning_uri: "otpauth://totp/example",
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          status: "authenticated",
          user: {
            id: "user-id",
            tenant_id: "tenant-id",
            tenant_slug: "development",
            tenant_name: "開発テナント",
            email: "admin@example.test",
            display_name: "開発管理者",
            role: "admin",
            mfa_enabled: true,
            csrf_token: "csrf",
          },
          recovery_codes: ["ABCDEF-GHIJKL", "MNOPQR-STUVWX"],
        }),
      );

    renderAuthPage();
    fireEvent.click(screen.getByRole("button", { name: "ログイン" }));

    expect(
      await screen.findByRole("heading", { name: "多要素認証の設定" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/JBSWY3DPEHPK3PXP/)).toBeInTheDocument();
    expect(
      await screen.findByRole("img", {
        name: "多要素認証セットアップ用QRコード",
      }),
    ).toHaveAttribute("src", "data:image/png;base64,test-qr");

    fireEvent.change(screen.getByLabelText(/6桁の認証コード/), {
      target: { value: "123456" },
    });
    fireEvent.click(screen.getByRole("button", { name: "MFAを有効にする" }));

    expect(
      await screen.findByRole("heading", { name: "リカバリーコードを保存" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/ABCDEF-GHIJKL/)).toBeInTheDocument();
  });
});

function renderAuthPage(initialEntry = "/") {
  render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <AuthPage onAuthenticated={() => undefined} />
    </MemoryRouter>,
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
