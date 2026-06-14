import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BillingPage } from "./BillingPage";
import type { CurrentUser } from "./api";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("BillingPage", () => {
  it("Stripe課金プランを表示する", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
      const url = String(input);
      if (url.endsWith("/v1/billing/plans")) {
        return jsonResponse([
          {
            id: "01900000-0000-7000-8000-000000000001",
            code: "standard",
            name: "スタンダード",
            description: "標準プラン",
            currency: "jpy",
            unit_amount: 3000,
            billing_interval: "month",
            active: true,
            stripe_product_id: "prod_test",
            stripe_price_id: "price_test",
          },
        ]);
      }
      if (url.endsWith("/v1/billing/status")) {
        return jsonResponse({
          plan: null,
          status: null,
          current_period_end: null,
          cancel_at_period_end: false,
          unit_amount: null,
          billing_interval: null,
          stripe_configured: true,
        });
      }
      if (url.endsWith("/v1/billing/invoices")) {
        return jsonResponse([
          {
            id: "in_test",
            number: "INV-0001",
            status: "paid",
            total: 3000,
            currency: "jpy",
            created_at: 1767225600,
            invoice_pdf: "https://pay.stripe.com/invoice/test/pdf",
          },
        ]);
      }
      return new Response(null, { status: 404 });
    });

    renderPage();

    expect(
      await screen.findByRole("heading", { name: "スタンダード" }),
    ).toBeInTheDocument();
    expect(screen.getByText("￥3,000")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Stripeで申し込む" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("契約中の有料プランはありません。"),
    ).toBeInTheDocument();
    expect(await screen.findByText("INV-0001")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "PDFをダウンロード" }),
    ).toHaveAttribute("href", "https://pay.stripe.com/invoice/test/pdf");
  });
});

const currentUser: CurrentUser = {
  id: "018f0000-0000-7000-8000-000000000002",
  tenant_id: "018f0000-0000-7000-8000-000000000001",
  tenant_slug: "development",
  tenant_name: "開発テナント",
  email: "admin@example.test",
  display_name: "開発管理者",
  role: "admin",
  mfa_enabled: true,
  csrf_token: "csrf",
};

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/billing"]}>
        <BillingPage currentUser={currentUser} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
