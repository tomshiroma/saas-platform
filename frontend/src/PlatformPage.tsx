import { zodResolver } from "@hookform/resolvers/zod";
import {
  Alert,
  AppBar,
  Avatar,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Container,
  Divider,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Drawer,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Paper,
  Stack,
  SvgIcon,
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
import { useState, type ReactNode } from "react";
import { useForm } from "react-hook-form";
import {
  Link,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
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
import { ColorModeButton } from "./ColorMode";
import { PlatformPlanManagement } from "./PlatformPlanManagement";

const drawerWidth = 280;

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
      <Box sx={{ position: "relative", minHeight: "100vh" }}>
        <Box sx={{ position: "fixed", top: 16, right: 16, zIndex: 1 }}>
          <ColorModeButton />
        </Box>
        <PlatformLogin
          onAuthenticated={(nextAdmin) => {
            setSessionAdmin(nextAdmin);
            queryClient.setQueryData(["platform-admin"], {
              admin: nextAdmin,
            });
          }}
        />
      </Box>
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
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const location = useLocation();
  const navigation = [
    {
      path: "/platform",
      label: "ダッシュボード",
      icon: <DashboardIcon />,
    },
    {
      path: "/platform/plans",
      label: "プラン管理",
      icon: <PlanIcon />,
    },
    {
      path: "/platform/tenants",
      label: "テナント管理",
      icon: <TenantIcon />,
    },
    {
      path: "/platform/audit-logs",
      label: "監査ログ",
      icon: <AuditIcon />,
    },
  ];
  const sidebar = (
    <Stack sx={{ height: "100%" }}>
      <Stack sx={{ px: 2.5, py: 2.5 }}>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
          <Box
            sx={{
              width: 36,
              height: 36,
              borderRadius: 1,
              display: "grid",
              placeItems: "center",
              bgcolor: "primary.main",
              color: "primary.contrastText",
            }}
          >
            <PlatformIcon />
          </Box>
          <Box sx={{ minWidth: 0 }}>
            <Typography variant="subtitle1" sx={{ fontWeight: 700 }}>
              SaaS運営管理
            </Typography>
          </Box>
        </Stack>
      </Stack>

      <Box sx={{ px: 2 }}>
        <List sx={{ display: "grid", gap: 0.5 }}>
          {navigation.map((item) => {
            const selected =
              item.path === "/platform"
                ? location.pathname === item.path
                : location.pathname.startsWith(item.path);
            return (
              <ListItemButton
                key={item.path}
                component={Link}
                to={item.path}
                selected={selected}
                aria-current={selected ? "page" : undefined}
                onClick={() => setMobileMenuOpen(false)}
                sx={{
                  borderRadius: 1,
                  px: 1.5,
                  py: 1,
                  color: "text.secondary",
                  "&.Mui-selected": {
                    bgcolor: "action.selected",
                    color: "primary.main",
                  },
                  "&.Mui-selected:hover": {
                    bgcolor: "action.selected",
                  },
                  "&:hover": {
                    bgcolor: "action.hover",
                  },
                }}
              >
                <ListItemIcon
                  sx={{
                    minWidth: 42,
                    color: selected ? "primary.main" : "text.secondary",
                  }}
                >
                  {item.icon}
                </ListItemIcon>
                <ListItemText
                  primary={item.label}
                  slotProps={{
                    primary: { sx: { fontWeight: 700, fontSize: 14 } },
                  }}
                />
              </ListItemButton>
            );
          })}
        </List>
      </Box>

      <Box sx={{ mt: "auto", p: 2 }}>
        <Divider sx={{ mb: 2 }} />
        <Paper
          elevation={0}
          sx={{
            p: 1.5,
            bgcolor: "transparent",
            border: "1px solid",
            borderColor: "divider",
          }}
        >
          <Stack direction="row" spacing={1.25} sx={{ alignItems: "center" }}>
            <Avatar
              sx={{
                width: 38,
                height: 38,
                bgcolor: "primary.main",
                color: "primary.contrastText",
                fontWeight: 800,
              }}
            >
              {admin.display_name.slice(0, 1)}
            </Avatar>
            <Box sx={{ minWidth: 0, flexGrow: 1 }}>
              <Typography variant="body2" sx={{ fontWeight: 700 }} noWrap>
                {admin.display_name}
              </Typography>
              <Typography
                variant="caption"
                color="text.secondary"
                noWrap
              >
                Platform administrator
              </Typography>
            </Box>
          </Stack>
          <Button
            fullWidth
            startIcon={<LogoutIcon />}
            onClick={() => void onLogout()}
            sx={{
              mt: 1.25,
              color: "text.secondary",
              justifyContent: "flex-start",
              "&:hover": {
                bgcolor: "action.hover",
              },
            }}
          >
            ログアウト
          </Button>
          <Stack
            direction="row"
            sx={{ mt: 0.5, alignItems: "center", justifyContent: "space-between" }}
          >
            <Typography
              variant="caption"
              sx={{ pl: 1, color: "text.secondary" }}
            >
              表示テーマ
            </Typography>
            <ColorModeButton />
          </Stack>
        </Paper>
      </Box>
    </Stack>
  );

  return (
    <Box sx={{ display: "flex", minHeight: "100vh" }}>
      <AppBar
        position="fixed"
        elevation={0}
        sx={(theme) => ({
          display: { md: "none" },
          bgcolor:
            theme.palette.mode === "dark"
              ? "rgba(7,17,31,0.9)"
              : "rgba(248,250,252,0.9)",
          color: "text.primary",
          borderBottom: "1px solid",
          borderColor: "divider",
        })}
      >
        <Toolbar>
          <IconButton
            edge="start"
            aria-label="運営メニューを開く"
            onClick={() => setMobileMenuOpen(true)}
            sx={{ mr: 1 }}
          >
            <MenuIcon />
          </IconButton>
          <Typography variant="subtitle1" sx={{ fontWeight: 800 }}>
            SaaS運営管理
          </Typography>
          <Box sx={{ ml: "auto" }}>
            <ColorModeButton />
          </Box>
        </Toolbar>
      </AppBar>

      <Box component="nav" aria-label="運営管理ナビゲーション">
        <Drawer
          variant="temporary"
          open={mobileMenuOpen}
          onClose={() => setMobileMenuOpen(false)}
          ModalProps={{ keepMounted: true }}
          sx={{
            display: { xs: "block", md: "none" },
            "& .MuiDrawer-paper": platformSidebarPaperStyles,
          }}
        >
          {sidebar}
        </Drawer>
        <Drawer
          variant="permanent"
          open
          sx={{
            display: { xs: "none", md: "block" },
            width: drawerWidth,
            flexShrink: 0,
            "& .MuiDrawer-paper": platformSidebarPaperStyles,
          }}
        >
          {sidebar}
        </Drawer>
      </Box>

      <Box
        component="main"
        sx={{
          flexGrow: 1,
          minWidth: 0,
          pt: { xs: 10, md: 0 },
          bgcolor: "background.default",
        }}
      >
        <Container maxWidth="xl" sx={{ px: { xs: 2, sm: 3, lg: 5 }, py: { xs: 3, md: 5 } }}>
          <Routes>
            <Route path="/platform" element={<PlatformDashboard />} />
            <Route
              path="/platform/plans"
              element={
                <PlatformSection
                  title="プラン管理"
                  description="顧客向け料金プランとStripe連携を管理します。"
                >
                  <PlatformPlanManagement admin={admin} />
                </PlatformSection>
              }
            />
            <Route
              path="/platform/tenants"
              element={<PlatformTenantsPage admin={admin} />}
            />
            <Route
              path="/platform/audit-logs"
              element={<PlatformAuditLogsPage />}
            />
            <Route path="*" element={<Navigate to="/platform" replace />} />
          </Routes>
        </Container>
      </Box>
    </Box>
  );
}

function PlatformDashboard() {
  const summary = useQuery({
    queryKey: ["platform-summary"],
    queryFn: api.platformSummary,
  });

  return (
    <PlatformSection
      title="運営ダッシュボード"
      description="顧客テナントの利用状況と稼働状態を確認します。"
    >
      <SummaryCards summary={summary} />
    </PlatformSection>
  );
}

function PlatformTenantsPage({ admin }: { admin: PlatformAdmin }) {
  const tenants = useQuery({
    queryKey: ["platform-tenants"],
    queryFn: api.platformTenants,
  });

  return (
    <PlatformSection
      title="テナント管理"
      description="顧客テナントの利用者数、稼働状態、停止理由を管理します。"
    >
      <TenantManagement admin={admin} tenants={tenants} />
    </PlatformSection>
  );
}

function PlatformAuditLogsPage() {
  const auditLogs = useQuery({
    queryKey: ["platform-audit-logs"],
    queryFn: api.platformAuditLogs,
  });

  return (
    <PlatformSection
      title="運営監査ログ"
      description="運営管理者による重要操作の履歴を確認します。"
    >
      <AuditLogList auditLogs={auditLogs} />
    </PlatformSection>
  );
}

function PlatformSection({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <Stack spacing={3}>
      <Box>
        <Typography component="h1" variant="h3" gutterBottom>
          {title}
        </Typography>
        <Typography color="text.secondary">{description}</Typography>
      </Box>
      {children}
    </Stack>
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

const platformSidebarPaperStyles = {
  width: drawerWidth,
  boxSizing: "border-box",
  borderRight: "1px solid",
  borderColor: "divider",
  bgcolor: "background.paper",
} as const;

function PlatformIcon() {
  return (
    <SvgIcon>
      <path d="M12 2 3 6v6c0 5.25 3.84 10.16 9 11 5.16-.84 9-5.75 9-11V6l-9-4Zm0 5a3 3 0 1 1 0 6 3 3 0 0 1 0-6Zm0 11.9a7.7 7.7 0 0 1-4.8-2.43c.06-1.59 3.2-2.47 4.8-2.47s4.74.88 4.8 2.47A7.7 7.7 0 0 1 12 18.9Z" />
    </SvgIcon>
  );
}

function DashboardIcon() {
  return (
    <SvgIcon>
      <path d="M3 13h8V3H3v10Zm0 8h8v-6H3v6Zm10 0h8V11h-8v10Zm0-18v6h8V3h-8Z" />
    </SvgIcon>
  );
}

function PlanIcon() {
  return (
    <SvgIcon>
      <path d="M20 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2Zm0 14H4v-5h16v5Zm0-9H4V6h16v3Z" />
    </SvgIcon>
  );
}

function TenantIcon() {
  return (
    <SvgIcon>
      <path d="M12 7V3H2v18h20V7H12ZM6 19H4v-2h2v2Zm0-4H4v-2h2v2Zm0-4H4V9h2v2Zm0-4H4V5h2v2Zm4 12H8v-2h2v2Zm0-4H8v-2h2v2Zm0-4H8V9h2v2Zm0-4H8V5h2v2Zm10 12h-8v-2h2v-2h-2v-2h2v-2h-2V9h8v10Zm-2-8h-2v2h2v-2Zm0 4h-2v2h2v-2Z" />
    </SvgIcon>
  );
}

function AuditIcon() {
  return (
    <SvgIcon>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6Zm1 17H7v-2h8v2Zm2-4H7v-2h10v2Zm-4-6V3.5L18.5 9H13Z" />
    </SvgIcon>
  );
}

function LogoutIcon() {
  return (
    <SvgIcon fontSize="small">
      <path d="M17 7l-1.41 1.41L18.17 11H8v2h10.17l-2.58 2.59L17 17l5-5-5-5ZM4 5h6V3H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h6v-2H4V5Z" />
    </SvgIcon>
  );
}

function MenuIcon() {
  return (
    <SvgIcon>
      <path d="M3 18h18v-2H3v2Zm0-5h18v-2H3v2Zm0-7v2h18V6H3Z" />
    </SvgIcon>
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
