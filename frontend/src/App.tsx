import {
  Alert,
  AppBar,
  Box,
  Button,
  CircularProgress,
  Container,
  Stack,
  Toolbar,
  Typography,
} from "@mui/material";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, Navigate, Route, Routes } from "react-router-dom";
import { api, type CurrentUser } from "./api";
import { AuthPage } from "./AuthPage";
import { TenantPage } from "./TenantPage";

export function App() {
  const queryClient = useQueryClient();
  const [sessionUser, setSessionUser] = useState<CurrentUser>();
  const currentUser = useQuery({
    queryKey: ["current-user"],
    queryFn: api.me,
    retry: false,
    enabled: sessionUser === undefined,
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

  return (
    <>
      <AppBar position="static">
        <Toolbar>
          <Typography variant="h6" sx={{ flexGrow: 1 }}>
            {user.tenant_name}
          </Typography>
          <Button color="inherit" component={Link} to="/">
            ホーム
          </Button>
          {user.role === "admin" && (
            <Button color="inherit" component={Link} to="/tenant">
              テナント管理
            </Button>
          )}
          <Button color="inherit" onClick={() => void logout()}>
            ログアウト
          </Button>
        </Toolbar>
      </AppBar>

      <Container component="main" maxWidth="md" sx={{ py: 5 }}>
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
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </Container>
    </>
  );
}

function Dashboard({ currentUser }: { currentUser: CurrentUser }) {
  return (
    <Stack spacing={3}>
      <Box>
        <Typography component="h1" variant="h3" gutterBottom>
          ダッシュボード
        </Typography>
        <Typography color="text.secondary">
          {currentUser.display_name} さんとしてログインしています。
        </Typography>
      </Box>
      <Alert severity="success">
        認証済みです。権限: {currentUser.role === "admin" ? "管理者" : "一般"}
      </Alert>
    </Stack>
  );
}
