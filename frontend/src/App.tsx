import {
  Alert,
  AppBar,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  Container,
  Stack,
  Toolbar,
  Typography,
} from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { Link, Route, Routes } from "react-router-dom";

type HealthResponse = {
  status: "ok";
  service: string;
  version: string;
};

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";

async function fetchHealth(): Promise<HealthResponse> {
  const response = await fetch(`${apiBaseUrl}/v1/health/ready`);

  if (!response.ok) {
    throw new Error(`Health check failed: ${response.status}`);
  }

  return (await response.json()) as HealthResponse;
}

function HomePage() {
  const health = useQuery({
    queryKey: ["health"],
    queryFn: fetchHealth,
  });

  return (
    <Container component="main" maxWidth="md" sx={{ py: 6 }}>
      <Stack spacing={3}>
        <Box>
          <Typography component="h1" variant="h3" gutterBottom>
            SaaS Platform
          </Typography>
          <Typography color="text.secondary">
            React、Rust、PostgreSQLのDocker開発環境です。
          </Typography>
        </Box>

        <Card>
          <CardContent>
            <Typography component="h2" variant="h5" gutterBottom>
              API接続状態
            </Typography>

            {health.isPending ? (
              <Stack direction="row" spacing={2} sx={{ alignItems: "center" }}>
                <CircularProgress size={24} />
                <Typography>確認中...</Typography>
              </Stack>
            ) : health.isError ? (
              <Alert severity="error">
                APIへ接続できません。コンテナログを確認してください。
              </Alert>
            ) : (
              <Alert severity="success">
                {health.data.service} {health.data.version} は正常です。
              </Alert>
            )}
          </CardContent>
        </Card>

        <Box>
          <Button variant="outlined" component={Link} to="/about">
            環境情報
          </Button>
        </Box>
      </Stack>
    </Container>
  );
}

function AboutPage() {
  return (
    <Container component="main" maxWidth="md" sx={{ py: 6 }}>
      <Stack spacing={3}>
        <Typography component="h1" variant="h3">
          環境情報
        </Typography>
        <Typography>
          フロントエンドはViteのHMR、バックエンドはcargo-watchで変更を検知します。
        </Typography>
        <Button component={Link} to="/" sx={{ alignSelf: "flex-start" }}>
          戻る
        </Button>
      </Stack>
    </Container>
  );
}

export function App() {
  return (
    <>
      <AppBar position="static">
        <Toolbar>
          <Typography variant="h6">SaaS Platform</Typography>
        </Toolbar>
      </AppBar>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/about" element={<AboutPage />} />
      </Routes>
    </>
  );
}
