import { zodResolver } from "@hookform/resolvers/zod";
import {
  Alert,
  AppBar,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Container,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Toolbar,
  Typography,
} from "@mui/material";
import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryResult,
} from "@tanstack/react-query";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import {
  ApiError,
  api,
  type PlatformAdmin,
  type PlatformAuditLog,
  type PlatformLoginInput,
  type PlatformSummary,
  type PlatformTenant,
} from "./api";

const platformLoginSchema = z.object({
  email: z.email("メールアドレスを入力してください。"),
  password: z.string().min(12, "12文字以上で入力してください。"),
  totp_code: z
    .string()
    .regex(/^\d{6}$/, "6桁の認証コードを入力してください。"),
});

export function PlatformPage() {
  const queryClient = useQueryClient();
  const [sessionAdmin, setSessionAdmin] = useState<PlatformAdmin>();
  const currentAdmin = useQuery({
    queryKey: ["platform-admin"],
    queryFn: api.platformMe,
    retry: false,
    enabled: sessionAdmin === undefined,
  });
  const admin = sessionAdmin ?? currentAdmin.data?.admin;

  if (currentAdmin.isPending && sessionAdmin === undefined) {
    return <FullPageLoading />;
  }
  if (admin === undefined) {
    return (
      <PlatformLogin
        onAuthenticated={(nextAdmin) => {
          setSessionAdmin(nextAdmin);
          queryClient.setQueryData(["platform-admin"], {
            admin: nextAdmin,
          });
        }}
      />
    );
  }

  const logout = async () => {
    await api.platformLogout(admin.csrf_token);
    setSessionAdmin(undefined);
    queryClient.clear();
  };

  return <PlatformConsole admin={admin} onLogout={logout} />;
}

function PlatformLogin({
  onAuthenticated,
}: {
  onAuthenticated: (admin: PlatformAdmin) => void;
}) {
  const [error, setError] = useState<string>();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<PlatformLoginInput>({
    resolver: zodResolver(platformLoginSchema),
    defaultValues: {
      email: "platform-admin@example.test",
      password: "platform-development-password",
      totp_code: "",
    },
  });

  const submit = handleSubmit(async (values) => {
    setError(undefined);
    try {
      const response = await api.platformLogin(values);
      onAuthenticated(response.admin);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  });

  return (
    <Container component="main" maxWidth="sm" sx={{ py: 8 }}>
      <Stack spacing={3}>
        <Box>
          <Typography component="h1" variant="h3" gutterBottom>
            SaaS運営管理
          </Typography>
          <Typography color="text.secondary">
            運営管理者アカウントとMFAでログインしてください。
          </Typography>
        </Box>
        <Card>
          <CardContent>
            <Stack component="form" spacing={2} onSubmit={submit} noValidate>
              {error !== undefined && <Alert severity="error">{error}</Alert>}
              <TextField
                label="メールアドレス"
                type="email"
                autoComplete="username"
                error={errors.email !== undefined}
                helperText={errors.email?.message}
                {...register("email")}
              />
              <TextField
                label="パスワード"
                type="password"
                autoComplete="current-password"
                error={errors.password !== undefined}
                helperText={errors.password?.message}
                {...register("password")}
              />
              <TextField
                label="認証コード"
                inputMode="numeric"
                autoComplete="one-time-code"
                error={errors.totp_code !== undefined}
                helperText={errors.totp_code?.message}
                {...register("totp_code")}
              />
              <Button type="submit" variant="contained" disabled={isSubmitting}>
                {isSubmitting ? "ログイン中..." : "運営管理へログイン"}
              </Button>
            </Stack>
          </CardContent>
        </Card>
        <Alert severity="info">
          開発用TOTPシークレット: <strong>JBSWY3DPEHPK3PXP</strong>
        </Alert>
      </Stack>
    </Container>
  );
}

function PlatformConsole({
  admin,
  onLogout,
}: {
  admin: PlatformAdmin;
  onLogout: () => Promise<void>;
}) {
  const summary = useQuery({
    queryKey: ["platform-summary"],
    queryFn: api.platformSummary,
  });
  const tenants = useQuery({
    queryKey: ["platform-tenants"],
    queryFn: api.platformTenants,
  });
  const auditLogs = useQuery({
    queryKey: ["platform-audit-logs"],
    queryFn: api.platformAuditLogs,
  });

  return (
    <>
      <AppBar position="static">
        <Toolbar>
          <Typography variant="h6" sx={{ flexGrow: 1 }}>
            SaaS運営管理
          </Typography>
          <Typography sx={{ mr: 2 }}>{admin.display_name}</Typography>
          <Button color="inherit" onClick={() => void onLogout()}>
            ログアウト
          </Button>
        </Toolbar>
      </AppBar>
      <Container component="main" maxWidth="lg" sx={{ py: 5 }}>
        <Stack spacing={4}>
          <Box>
            <Typography component="h1" variant="h3" gutterBottom>
              運営ダッシュボード
            </Typography>
            <Typography color="text.secondary">
              顧客テナントの利用状況と稼働状態を管理します。
            </Typography>
          </Box>
          <SummaryCards summary={summary} />
          <TenantManagement admin={admin} tenants={tenants} />
          <AuditLogList auditLogs={auditLogs} />
        </Stack>
      </Container>
    </>
  );
}

function SummaryCards({
  summary,
}: {
  summary: UseQueryResult<PlatformSummary, Error>;
}) {
  if (summary.isPending) {
    return <CircularProgress />;
  }
  if (summary.isError) {
    return <Alert severity="error">{errorMessage(summary.error)}</Alert>;
  }
  const cards = [
    ["総テナント", summary.data.tenant_count],
    ["稼働中", summary.data.active_tenant_count],
    ["停止中", summary.data.suspended_tenant_count],
    ["有効ユーザー", summary.data.active_user_count],
  ] as const;

  return (
    <Box
      sx={{
        display: "grid",
        gap: 2,
        gridTemplateColumns: { xs: "1fr", sm: "repeat(2, 1fr)", md: "repeat(4, 1fr)" },
      }}
    >
      {cards.map(([label, value]) => (
        <Card key={label}>
          <CardContent>
            <Typography color="text.secondary">{label}</Typography>
            <Typography variant="h4">{value}</Typography>
          </CardContent>
        </Card>
      ))}
    </Box>
  );
}

function TenantManagement({
  admin,
  tenants,
}: {
  admin: PlatformAdmin;
  tenants: UseQueryResult<PlatformTenant[], Error>;
}) {
  if (tenants.isPending) {
    return <CircularProgress />;
  }
  if (tenants.isError) {
    return <Alert severity="error">{errorMessage(tenants.error)}</Alert>;
  }

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Typography component="h2" variant="h5">
            テナント管理
          </Typography>
          {tenants.data.length === 0 ? (
            <Alert severity="info">テナントはありません。</Alert>
          ) : (
            tenants.data.map((tenant) => (
              <PlatformTenantRow
                key={tenant.id}
                tenant={tenant}
                admin={admin}
              />
            ))
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}

function PlatformTenantRow({
  tenant,
  admin,
}: {
  tenant: PlatformTenant;
  admin: PlatformAdmin;
}) {
  const queryClient = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string>();
  const mutation = useMutation({
    mutationFn: () =>
      api.updatePlatformTenant(
        tenant.id,
        {
          active: !tenant.active,
          suspension_reason: tenant.active ? reason : undefined,
        },
        admin.csrf_token,
      ),
    onSuccess: async () => {
      setDialogOpen(false);
      setReason("");
      setError(undefined);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["platform-summary"] }),
        queryClient.invalidateQueries({ queryKey: ["platform-tenants"] }),
        queryClient.invalidateQueries({ queryKey: ["platform-audit-logs"] }),
      ]);
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  return (
    <Box sx={{ border: 1, borderColor: "divider", borderRadius: 1, p: 2 }}>
      <Stack spacing={1}>
        <Stack direction={{ xs: "column", sm: "row" }} spacing={1}>
          <Typography sx={{ fontWeight: 700, flexGrow: 1 }}>
            {tenant.name}
          </Typography>
          <Chip
            color={tenant.active ? "success" : "error"}
            label={tenant.active ? "稼働中" : "停止中"}
          />
        </Stack>
        <Typography color="text.secondary">
          ID: {tenant.slug} / ユーザー: {tenant.active_user_count}名
          （登録 {tenant.user_count}名）
        </Typography>
        {!tenant.active && tenant.suspension_reason !== undefined && (
          <Alert severity="warning">
            停止理由: {tenant.suspension_reason}
          </Alert>
        )}
        {error !== undefined && <Alert severity="error">{error}</Alert>}
        <Button
          color={tenant.active ? "error" : "primary"}
          variant="outlined"
          sx={{ alignSelf: "flex-start" }}
          onClick={() => {
            if (tenant.active) {
              setDialogOpen(true);
            } else {
              mutation.mutate();
            }
          }}
          disabled={mutation.isPending}
        >
          {tenant.active ? "テナントを停止" : "テナントを再開"}
        </Button>
      </Stack>
      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} fullWidth>
        <DialogTitle>テナントを停止</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <Alert severity="warning">
              停止すると、このテナントの全セッションが直ちに失効します。
            </Alert>
            <TextField
              label="停止理由"
              value={reason}
              onChange={(event) => setReason(event.target.value)}
              multiline
              minRows={3}
              slotProps={{ htmlInput: { maxLength: 500 } }}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDialogOpen(false)}>キャンセル</Button>
          <Button
            color="error"
            variant="contained"
            onClick={() => mutation.mutate()}
            disabled={mutation.isPending || reason.trim().length === 0}
          >
            停止する
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}

function AuditLogList({
  auditLogs,
}: {
  auditLogs: UseQueryResult<PlatformAuditLog[], Error>;
}) {
  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Typography component="h2" variant="h5">
            運営監査ログ
          </Typography>
          {auditLogs.isPending ? (
            <CircularProgress />
          ) : auditLogs.isError ? (
            <Alert severity="error">{errorMessage(auditLogs.error)}</Alert>
          ) : auditLogs.data.length === 0 ? (
            <Alert severity="info">監査ログはありません。</Alert>
          ) : (
            auditLogs.data.map((log) => (
              <Box key={log.id} sx={{ borderBottom: 1, borderColor: "divider", pb: 1 }}>
                <Typography sx={{ fontWeight: 600 }}>{log.action}</Typography>
                <Typography color="text.secondary" variant="body2">
                  {log.actor_display_name ?? "不明"} / {formatDate(log.created_at)}
                </Typography>
              </Box>
            ))
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}

function FullPageLoading() {
  return (
    <Stack sx={{ minHeight: "100vh", alignItems: "center", justifyContent: "center" }}>
      <CircularProgress />
    </Stack>
  );
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "Asia/Tokyo",
  }).format(new Date(value));
}

function errorMessage(cause: unknown): string {
  return cause instanceof ApiError ? cause.message : "通信に失敗しました。";
}
