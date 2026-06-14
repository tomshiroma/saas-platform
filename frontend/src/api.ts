export type Role = "admin" | "member";

export type CurrentUser = {
  id: string;
  tenant_id: string;
  tenant_slug: string;
  tenant_name: string;
  email: string;
  display_name: string;
  role: Role;
  csrf_token: string;
};

export type Tenant = {
  id: string;
  slug: string;
  name: string;
};

export type TenantUser = {
  id: string;
  email: string;
  display_name: string;
  role: Role;
  active: boolean;
};

export type LoginInput = {
  tenant_slug: string;
  email: string;
  password: string;
};

export type RegisterInput = LoginInput & {
  tenant_name: string;
  display_name: string;
};

export type PasswordResetRequestInput = Pick<
  LoginInput,
  "tenant_slug" | "email"
>;

export type PasswordResetConfirmInput = {
  token: string;
  password: string;
};

export type CreateUserInput = {
  email: string;
  display_name: string;
  role: Role;
  password: string;
};

export type PlatformAdmin = {
  id: string;
  email: string;
  display_name: string;
  csrf_token: string;
};

export type PlatformLoginInput = {
  email: string;
  password: string;
  totp_code: string;
};

export type PlatformSummary = {
  tenant_count: number;
  active_tenant_count: number;
  suspended_tenant_count: number;
  user_count: number;
  active_user_count: number;
};

export type PlatformTenant = {
  id: string;
  slug: string;
  name: string;
  active: boolean;
  suspension_reason?: string;
  created_at: string;
  user_count: number;
  active_user_count: number;
};

export type PlatformAuditLog = {
  id: string;
  actor_display_name?: string;
  action: string;
  target_type: string;
  target_id?: string;
  details: Record<string, unknown>;
  created_at: string;
};

export type BillingInterval = "month" | "year";

export type BillingPlan = {
  id: string;
  code: string;
  name: string;
  description: string;
  currency: "jpy";
  unit_amount: number;
  billing_interval: BillingInterval;
  active: boolean;
  stripe_product_id: string;
  stripe_price_id: string;
};

export type CreateBillingPlanInput = Pick<
  BillingPlan,
  "code" | "name" | "description" | "unit_amount" | "billing_interval"
>;

export type BillingStatus = {
  plan: BillingPlan | null;
  status: string | null;
  current_period_end: string | null;
  cancel_at_period_end: boolean;
  unit_amount: number | null;
  billing_interval: BillingInterval | null;
  stripe_configured: boolean;
};

export type BillingInvoice = {
  id: string;
  number: string | null;
  status: string | null;
  total: number;
  currency: string;
  created_at: number;
  invoice_pdf: string | null;
};

type AuthResponse = {
  user: CurrentUser;
};

type PlatformAuthResponse = {
  admin: PlatformAdmin;
};

type ApiErrorBody = {
  code?: string;
  message?: string;
  request_id?: string;
};

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly requestId?: string;

  constructor(status: number, body: ApiErrorBody) {
    super(body.message ?? "APIリクエストに失敗しました。");
    this.name = "ApiError";
    this.status = status;
    this.code = body.code ?? "UNKNOWN_ERROR";
    this.requestId = body.request_id;
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  csrfToken?: string,
): Promise<T> {
  const headers = new Headers(options.headers);
  if (options.body !== undefined) {
    headers.set("Content-Type", "application/json");
  }
  if (csrfToken !== undefined) {
    headers.set("X-CSRF-Token", csrfToken);
  }

  const response = await fetch(`${apiBaseUrl}${path}`, {
    ...options,
    credentials: "same-origin",
    headers,
  });

  if (!response.ok) {
    const body = (await response
      .json()
      .catch(() => ({}))) as ApiErrorBody;
    throw new ApiError(response.status, body);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export const api = {
  me: () => request<AuthResponse>("/v1/auth/me"),
  login: (input: LoginInput) =>
    request<AuthResponse>("/v1/auth/login", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  register: (input: RegisterInput) =>
    request<AuthResponse>("/v1/auth/register", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  requestPasswordReset: (input: PasswordResetRequestInput) =>
    request<{ message: string }>("/v1/auth/password-reset/request", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  confirmPasswordReset: (input: PasswordResetConfirmInput) =>
    request<void>("/v1/auth/password-reset/confirm", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  logout: (csrfToken: string) =>
    request<void>("/v1/auth/logout", { method: "POST" }, csrfToken),
  tenant: () => request<Tenant>("/v1/tenant"),
  updateTenant: (name: string, csrfToken: string) =>
    request<Tenant>(
      "/v1/tenant",
      { method: "PATCH", body: JSON.stringify({ name }) },
      csrfToken,
    ),
  users: () => request<TenantUser[]>("/v1/tenant/users"),
  createUser: (input: CreateUserInput, csrfToken: string) =>
    request<TenantUser>(
      "/v1/tenant/users",
      { method: "POST", body: JSON.stringify(input) },
      csrfToken,
    ),
  updateUser: (
    userId: string,
    input: Pick<TenantUser, "display_name" | "role" | "active">,
    csrfToken: string,
  ) =>
    request<TenantUser>(
      `/v1/tenant/users/${userId}`,
      { method: "PATCH", body: JSON.stringify(input) },
      csrfToken,
    ),
  deleteUser: (userId: string, csrfToken: string) =>
    request<void>(
      `/v1/tenant/users/${userId}`,
      { method: "DELETE" },
      csrfToken,
    ),
  platformMe: () =>
    request<PlatformAuthResponse>("/v1/platform/auth/me"),
  platformLogin: (input: PlatformLoginInput) =>
    request<PlatformAuthResponse>("/v1/platform/auth/login", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  platformLogout: (csrfToken: string) =>
    request<void>(
      "/v1/platform/auth/logout",
      { method: "POST" },
      csrfToken,
    ),
  platformSummary: () =>
    request<PlatformSummary>("/v1/platform/summary"),
  platformTenants: () =>
    request<PlatformTenant[]>("/v1/platform/tenants"),
  updatePlatformTenant: (
    tenantId: string,
    input: Pick<PlatformTenant, "active" | "suspension_reason">,
    csrfToken: string,
  ) =>
    request<PlatformTenant>(
      `/v1/platform/tenants/${tenantId}`,
      { method: "PATCH", body: JSON.stringify(input) },
      csrfToken,
    ),
  platformAuditLogs: () =>
    request<PlatformAuditLog[]>("/v1/platform/audit-logs"),
  platformPlans: () =>
    request<BillingPlan[]>("/v1/platform/plans"),
  createPlatformPlan: (
    input: CreateBillingPlanInput,
    csrfToken: string,
  ) =>
    request<BillingPlan>(
      "/v1/platform/plans",
      { method: "POST", body: JSON.stringify(input) },
      csrfToken,
    ),
  updatePlatformPlan: (
    planId: string,
    input: Omit<CreateBillingPlanInput, "code"> & { active: boolean },
    csrfToken: string,
  ) =>
    request<BillingPlan>(
      `/v1/platform/plans/${planId}`,
      { method: "PATCH", body: JSON.stringify(input) },
      csrfToken,
    ),
  billingPlans: () => request<BillingPlan[]>("/v1/billing/plans"),
  billingStatus: () => request<BillingStatus>("/v1/billing/status"),
  billingInvoices: () =>
    request<BillingInvoice[]>("/v1/billing/invoices"),
  createCheckout: (planId: string, csrfToken: string) =>
    request<{ url: string }>(
      "/v1/billing/checkout",
      { method: "POST", body: JSON.stringify({ plan_id: planId }) },
      csrfToken,
    ),
  createBillingPortal: (csrfToken: string) =>
    request<{ url: string }>(
      "/v1/billing/portal",
      { method: "POST" },
      csrfToken,
    ),
};
