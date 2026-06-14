import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PlatformPage } from "./PlatformPage";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("PlatformPage", () => {
  it("未認証の場合にMFAログイン画面を表示する", async () => {
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

    renderPage();

    expect(
      await screen.findByRole("heading", { name: "SaaS運営管理" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("認証コード")).toBeInTheDocument();
  });

  it("認証済みの場合に運営ダッシュボードを表示する", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
      const url = String(input);
      if (url.endsWith("/v1/platform/auth/me")) {
        return jsonResponse({
          admin: {
            id: "018f0000-0000-7000-8000-000000000010",
            email: "platform-admin@example.test",
            display_name: "SaaS運営管理者",
            csrf_token: "csrf",
          },
        });
      }
      if (url.endsWith("/v1/platform/summary")) {
        return jsonResponse({
          tenant_count: 1,
          active_tenant_count: 1,
          suspended_tenant_count: 0,
          user_count: 2,
          active_user_count: 2,
        });
      }
      if (url.endsWith("/v1/platform/tenants")) {
        return jsonResponse([]);
      }
      if (url.endsWith("/v1/platform/plans")) {
        return jsonResponse([]);
      }
      if (url.endsWith("/v1/platform/audit-logs")) {
        return jsonResponse([]);
      }
      return new Response(null, { status: 404 });
    });

    renderPage();

    expect(
      await screen.findByRole("heading", { name: "運営ダッシュボード" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("総テナント")).toBeInTheDocument();
    expect(screen.getAllByText("SaaS運営管理者")).not.toHaveLength(0);
    expect(
      screen.getByRole("navigation", { name: "運営管理ナビゲーション" }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("link", { name: /ダッシュボード/ })[0],
    ).toHaveAttribute("aria-current", "page");
    expect(
      screen.getAllByRole("button", { name: "ダークモードに切り替え" }),
    ).not.toHaveLength(0);
    expect(
      fetchMock.mock.calls.some(([input]) =>
        String(input).endsWith("/v1/platform/plans"),
      ),
    ).toBe(false);
  });

  it.each([
    ["/platform/plans", "プラン管理", "プラン管理", "/v1/platform/plans"],
    ["/platform/tenants", "テナント管理", "テナント管理", "/v1/platform/tenants"],
    ["/platform/audit-logs", "運営監査ログ", "監査ログ", "/v1/platform/audit-logs"],
  ])("%sで独立した管理画面を表示する", async (path, heading, menuLabel, endpoint) => {
    const fetchMock = mockAuthenticatedRequests();

    renderPage(path);

    expect(
      await screen.findByRole("heading", { name: heading }),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(([input]) => String(input).endsWith(endpoint)),
    ).toBe(true);
    expect(
      screen.getAllByRole("link", { name: new RegExp(menuLabel) })[0],
    ).toHaveAttribute("aria-current", "page");
  });
});

function renderPage(initialEntry = "/platform") {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialEntry]}>
        <PlatformPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function mockAuthenticatedRequests() {
  return vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = String(input);
    if (url.endsWith("/v1/platform/auth/me")) {
      return jsonResponse({
        admin: {
          id: "018f0000-0000-7000-8000-000000000010",
          email: "platform-admin@example.test",
          display_name: "SaaS運営管理者",
          csrf_token: "csrf",
        },
      });
    }
    if (url.endsWith("/v1/platform/summary")) {
      return jsonResponse({
        tenant_count: 1,
        active_tenant_count: 1,
        suspended_tenant_count: 0,
        user_count: 2,
        active_user_count: 2,
      });
    }
    if (
      url.endsWith("/v1/platform/tenants") ||
      url.endsWith("/v1/platform/plans") ||
      url.endsWith("/v1/platform/audit-logs")
    ) {
      return jsonResponse([]);
    }
    return new Response(null, { status: 404 });
  });
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
