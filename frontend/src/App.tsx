import {
  Alert,
  AppBar,
  Avatar,
  Box,
  Button,
  CircularProgress,
  Container,
  Divider,
  Drawer,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Paper,
  Stack,
  SvgIcon,
  Toolbar,
  Typography,
} from "@mui/material";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useState } from "react";
import {
  Link,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { api, type CurrentUser } from "./api";
import { AuthPage } from "./AuthPage";
import { BillingPage } from "./BillingPage";
import { TenantPage } from "./TenantPage";

const PlatformPage = lazy(() =>
  import("./PlatformPage").then((module) => ({
    default: module.PlatformPage,
  })),
);

const drawerWidth = 280;

export function App() {
  const location = useLocation();
  if (location.pathname.startsWith("/platform")) {
    return (
      <Suspense fallback={<FullPageLoading />}>
        <PlatformPage />
      </Suspense>
    );
  }
  return <CustomerApp />;
}

function CustomerApp() {
  const location = useLocation();
  const isPasswordReset = location.pathname === "/reset-password";
  const queryClient = useQueryClient();
  const [sessionUser, setSessionUser] = useState<CurrentUser>();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const currentUser = useQuery({
    queryKey: ["current-user"],
    queryFn: api.me,
    retry: false,
    enabled: sessionUser === undefined && !isPasswordReset,
  });

  const user = sessionUser ?? currentUser.data?.user;

  const setUser = (nextUser: CurrentUser) => {
    setSessionUser(nextUser);
    queryClient.setQueryData(["current-user"], { user: nextUser });
  };

  const logout = async () => {
    if (user === undefined) {
      return;
    }
    await api.logout(user.csrf_token);
    setSessionUser(undefined);
    queryClient.clear();
  };

  if (isPasswordReset) {
    return <AuthPage onAuthenticated={setUser} />;
  }

  if (currentUser.isPending && sessionUser === undefined) {
    return (
      <Stack sx={{ minHeight: "100vh", alignItems: "center", justifyContent: "center" }}>
        <CircularProgress />
      </Stack>
    );
  }

  if (user === undefined) {
    return <AuthPage onAuthenticated={setUser} />;
  }

  const navigation = [
    {
      label: "ダッシュボード",
      description: "利用状況の概要",
      path: "/",
      icon: <DashboardIcon />,
    },
    ...(user.role === "admin"
      ? [
          {
            label: "テナント管理",
            description: "設定とメンバー",
            path: "/tenant",
            icon: <TenantIcon />,
          },
          {
            label: "契約・お支払い",
            description: "プランと請求",
            path: "/billing",
            icon: <BillingIcon />,
          },
        ]
      : []),
  ];
  const sidebar = (
    <Stack sx={{ height: "100%", color: "common.white" }}>
      <Stack spacing={0.5} sx={{ px: 3, pt: 3.5, pb: 3 }}>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
          <Box
            sx={{
              width: 42,
              height: 42,
              borderRadius: 2.5,
              display: "grid",
              placeItems: "center",
              background:
                "linear-gradient(135deg, rgba(110, 231, 183, 1), rgba(56, 189, 248, 1))",
              color: "#082f49",
              boxShadow: "0 10px 30px rgba(56, 189, 248, 0.24)",
            }}
          >
            <BrandIcon />
          </Box>
          <Box sx={{ minWidth: 0 }}>
            <Typography variant="subtitle1" sx={{ fontWeight: 800, lineHeight: 1.2 }}>
              SaaS Platform
            </Typography>
            <Typography
              variant="caption"
              sx={{ color: "rgba(255,255,255,0.58)" }}
              noWrap
            >
              Workspace Console
            </Typography>
          </Box>
        </Stack>
      </Stack>

      <Box sx={{ px: 2 }}>
        <Typography
          variant="overline"
          sx={{
            px: 1.5,
            color: "rgba(255,255,255,0.42)",
            fontWeight: 700,
            letterSpacing: "0.14em",
          }}
        >
          Menu
        </Typography>
        <List sx={{ mt: 0.5, display: "grid", gap: 0.75 }}>
          {navigation.map((item) => {
            const selected =
              item.path === "/"
                ? location.pathname === "/"
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
                  borderRadius: 2.5,
                  px: 1.5,
                  py: 1.2,
                  color: selected ? "common.white" : "rgba(255,255,255,0.68)",
                  "&.Mui-selected": {
                    bgcolor: "rgba(255,255,255,0.11)",
                    boxShadow: "inset 3px 0 0 #6ee7b7",
                  },
                  "&.Mui-selected:hover": {
                    bgcolor: "rgba(255,255,255,0.14)",
                  },
                  "&:hover": {
                    bgcolor: "rgba(255,255,255,0.07)",
                    color: "common.white",
                  },
                }}
              >
                <ListItemIcon
                  sx={{
                    minWidth: 42,
                    color: selected ? "#6ee7b7" : "rgba(255,255,255,0.48)",
                  }}
                >
                  {item.icon}
                </ListItemIcon>
                <ListItemText
                  primary={item.label}
                  secondary={item.description}
                  slotProps={{
                    primary: { sx: { fontWeight: 700, fontSize: 14 } },
                    secondary: {
                      sx: {
                        color: "rgba(255,255,255,0.4)",
                        fontSize: 11,
                      },
                    },
                  }}
                />
              </ListItemButton>
            );
          })}
        </List>
      </Box>

      <Box sx={{ mt: "auto", p: 2 }}>
        <Divider sx={{ borderColor: "rgba(255,255,255,0.09)", mb: 2 }} />
        <Paper
          elevation={0}
          sx={{
            p: 1.5,
            bgcolor: "rgba(255,255,255,0.07)",
            color: "common.white",
            border: "1px solid rgba(255,255,255,0.08)",
          }}
        >
          <Stack direction="row" spacing={1.25} sx={{ alignItems: "center" }}>
            <Avatar
              sx={{
                width: 38,
                height: 38,
                bgcolor: "rgba(110,231,183,0.16)",
                color: "#6ee7b7",
                fontWeight: 800,
              }}
            >
              {user.display_name.slice(0, 1)}
            </Avatar>
            <Box sx={{ minWidth: 0, flexGrow: 1 }}>
              <Typography variant="body2" sx={{ fontWeight: 700 }} noWrap>
                {user.display_name}
              </Typography>
              <Typography
                variant="caption"
                sx={{ color: "rgba(255,255,255,0.46)" }}
                noWrap
              >
                {user.tenant_name}
              </Typography>
            </Box>
          </Stack>
          <Button
            fullWidth
            startIcon={<LogoutIcon />}
            onClick={() => void logout()}
            sx={{
              mt: 1.25,
              color: "rgba(255,255,255,0.72)",
              justifyContent: "flex-start",
              "&:hover": {
                color: "common.white",
                bgcolor: "rgba(255,255,255,0.07)",
              },
            }}
          >
            ログアウト
          </Button>
        </Paper>
      </Box>
    </Stack>
  );

  return (
    <Box sx={{ display: "flex", minHeight: "100vh" }}>
      <AppBar
        position="fixed"
        elevation={0}
        sx={{
          display: { md: "none" },
          bgcolor: "rgba(248,250,252,0.9)",
          color: "text.primary",
          backdropFilter: "blur(16px)",
          borderBottom: "1px solid",
          borderColor: "divider",
        }}
      >
        <Toolbar>
          <IconButton
            edge="start"
            aria-label="メニューを開く"
            onClick={() => setMobileMenuOpen(true)}
            sx={{ mr: 1 }}
          >
            <MenuIcon />
          </IconButton>
          <Typography variant="subtitle1" sx={{ fontWeight: 800 }}>
            {user.tenant_name}
          </Typography>
        </Toolbar>
      </AppBar>

      <Box component="nav" aria-label="メインナビゲーション">
        <Drawer
          variant="temporary"
          open={mobileMenuOpen}
          onClose={() => setMobileMenuOpen(false)}
          ModalProps={{ keepMounted: true }}
          sx={{
            display: { xs: "block", md: "none" },
            "& .MuiDrawer-paper": sidebarPaperStyles,
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
            "& .MuiDrawer-paper": sidebarPaperStyles,
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
          background:
            "radial-gradient(circle at 90% 0%, rgba(56,189,248,0.09), transparent 28rem), #f8fafc",
        }}
      >
        <Container maxWidth="lg" sx={{ px: { xs: 2, sm: 3, lg: 5 }, py: { xs: 3, md: 5 } }}>
        <Routes>
          <Route path="/" element={<Dashboard currentUser={user} />} />
          <Route
            path="/tenant"
            element={
              <TenantPage
                currentUser={user}
                onCurrentUserChanged={setUser}
              />
            }
          />
          <Route
            path="/billing"
            element={<BillingPage currentUser={user} />}
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
        </Container>
      </Box>
    </Box>
  );
}

function Dashboard({ currentUser }: { currentUser: CurrentUser }) {
  return (
    <Stack spacing={4}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={2}
        sx={{ justifyContent: "space-between", alignItems: { sm: "flex-end" } }}
      >
        <Box>
          <Typography
            variant="overline"
            color="primary.main"
            sx={{ fontWeight: 800, letterSpacing: "0.13em" }}
          >
            Overview
          </Typography>
          <Typography component="h1" variant="h3" sx={{ mt: 0.5 }}>
            ダッシュボード
          </Typography>
          <Typography color="text.secondary" sx={{ mt: 1 }}>
            {currentUser.display_name} さん、おかえりなさい。
          </Typography>
        </Box>
        <Box
          sx={{
            px: 1.5,
            py: 0.75,
            borderRadius: 99,
            bgcolor: "success.light",
            color: "success.dark",
            fontSize: 13,
            fontWeight: 800,
          }}
        >
          システム正常
        </Box>
      </Stack>

      <Paper
        sx={{
          position: "relative",
          overflow: "hidden",
          p: { xs: 3, md: 4 },
          color: "common.white",
          background: "linear-gradient(125deg, #0f172a 0%, #164e63 100%)",
        }}
      >
        <Box
          aria-hidden="true"
          sx={{
            position: "absolute",
            width: 240,
            height: 240,
            borderRadius: "50%",
            right: -70,
            top: -110,
            background: "rgba(110,231,183,0.14)",
            filter: "blur(2px)",
          }}
        />
        <Stack spacing={2} sx={{ position: "relative", maxWidth: 620 }}>
          <Typography variant="overline" sx={{ color: "#6ee7b7", fontWeight: 800 }}>
            Current workspace
          </Typography>
          <Typography variant="h4" sx={{ fontWeight: 800 }}>
            {currentUser.tenant_name}
          </Typography>
          <Typography sx={{ color: "rgba(255,255,255,0.68)" }}>
            チームのメンバー、契約情報、ワークスペース設定を一つの場所から管理できます。
          </Typography>
          <Stack direction="row" spacing={1} sx={{ pt: 1 }}>
            <Box sx={dashboardStatStyles}>
              <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.5)" }}>
                あなたの権限
              </Typography>
              <Typography sx={{ fontWeight: 800 }}>
                {currentUser.role === "admin" ? "管理者" : "一般メンバー"}
              </Typography>
            </Box>
            <Box sx={dashboardStatStyles}>
              <Typography variant="caption" sx={{ color: "rgba(255,255,255,0.5)" }}>
                ステータス
              </Typography>
              <Typography sx={{ fontWeight: 800 }}>認証済み</Typography>
            </Box>
          </Stack>
        </Stack>
      </Paper>

      <Alert severity="success">
        安全にログインしています。操作内容はアカウントの権限に基づいて制御されます。
      </Alert>
    </Stack>
  );
}

const sidebarPaperStyles = {
  width: drawerWidth,
  boxSizing: "border-box",
  border: 0,
  background:
    "radial-gradient(circle at 20% 0%, rgba(14,116,144,0.48), transparent 19rem), #0f172a",
} as const;

const dashboardStatStyles = {
  minWidth: 132,
  px: 2,
  py: 1.25,
  borderRadius: 2,
  bgcolor: "rgba(255,255,255,0.07)",
  border: "1px solid rgba(255,255,255,0.08)",
} as const;

function BrandIcon() {
  return (
    <SvgIcon>
      <path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h4A1.5 1.5 0 0 1 11 5.5v4A1.5 1.5 0 0 1 9.5 11h-4A1.5 1.5 0 0 1 4 9.5v-4Zm9 0A1.5 1.5 0 0 1 14.5 4h4A1.5 1.5 0 0 1 20 5.5v4a1.5 1.5 0 0 1-1.5 1.5h-4A1.5 1.5 0 0 1 13 9.5v-4Zm-9 9A1.5 1.5 0 0 1 5.5 13h4a1.5 1.5 0 0 1 1.5 1.5v4A1.5 1.5 0 0 1 9.5 20h-4A1.5 1.5 0 0 1 4 18.5v-4Zm9 0a1.5 1.5 0 0 1 1.5-1.5h4a1.5 1.5 0 0 1 1.5 1.5v4a1.5 1.5 0 0 1-1.5 1.5h-4a1.5 1.5 0 0 1-1.5-1.5v-4Z" />
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

function TenantIcon() {
  return (
    <SvgIcon>
      <path d="M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-.32 0-.63.05-.91.14A5 5 0 0 1 16 8c0 1.07-.34 2.06-.91 2.86.28.09.59.14.91.14Zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5 5 6.34 5 8s1.34 3 3 3Zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5C15 14.17 10.33 13 8 13Zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5Z" />
    </SvgIcon>
  );
}

function BillingIcon() {
  return (
    <SvgIcon>
      <path d="M20 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2Zm0 14H4v-6h16v6Zm0-10H4V6h16v2Z" />
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
