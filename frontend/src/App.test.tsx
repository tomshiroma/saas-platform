import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("App", () => {
  it("未認証の場合にログイン画面を表示する", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          code: "UNAUTHORIZED",
          message: "認証が必要です。",
        }),
        {
          status: 401,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    renderApp();

    expect(await screen.findByRole("heading", { name: "SaaS Platform" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ログイン" })).toBeInTheDocument();
  });

  it("認証済みの場合にダッシュボードを表示する", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          user: {
            id: "018f0000-0000-7000-8000-000000000002",
            tenant_id: "018f0000-0000-7000-8000-000000000001",
            tenant_slug: "development",
            tenant_name: "開発テナント",
            email: "admin@example.test",
            display_name: "開発管理者",
            role: "admin",
            csrf_token: "csrf",
          },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    renderApp();

    expect(await screen.findByRole("heading", { name: "ダッシュボード" })).toBeInTheDocument();
    expect(screen.getByText("開発管理者 さん、おかえりなさい。")).toBeInTheDocument();
    expect(
      screen.getByRole("navigation", { name: "メインナビゲーション" }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("link", { name: /ダッシュボード/ })[0],
    ).toHaveAttribute("aria-current", "page");
  });
});

function renderApp() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
