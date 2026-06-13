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

export type CreateUserInput = {
  email: string;
  display_name: string;
  role: Role;
  password: string;
};

type AuthResponse = {
  user: CurrentUser;
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
};
