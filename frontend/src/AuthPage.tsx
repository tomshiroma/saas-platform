import { zodResolver } from "@hookform/resolvers/zod";
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Container,
  Stack,
  Tab,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { ApiError, api, type CurrentUser } from "./api";

const loginSchema = z.object({
  tenant_slug: z
    .string()
    .min(4, "4文字以上で入力してください。")
    .regex(/^[a-z0-9][a-z0-9-]*[a-z0-9]$/, "英小文字、数字、ハイフンで入力してください。"),
  email: z.email("メールアドレスを入力してください。"),
  password: z.string().min(12, "12文字以上で入力してください。"),
});

const registerSchema = loginSchema.extend({
  tenant_name: z.string().trim().min(1, "テナント名を入力してください。").max(100),
  display_name: z.string().trim().min(1, "表示名を入力してください。").max(100),
});

type LoginValues = z.infer<typeof loginSchema>;
type RegisterValues = z.infer<typeof registerSchema>;

type AuthPageProps = {
  onAuthenticated: (user: CurrentUser) => void;
};

export function AuthPage({ onAuthenticated }: AuthPageProps) {
  const [tab, setTab] = useState<"login" | "register">("login");

  return (
    <Container component="main" maxWidth="sm" sx={{ py: 8 }}>
      <Stack spacing={3}>
        <Box>
          <Typography component="h1" variant="h3" gutterBottom>
            SaaS Platform
          </Typography>
          <Typography color="text.secondary">
            テナントIDを指定してログインしてください。
          </Typography>
        </Box>

        <Card>
          <Tabs
            value={tab}
            onChange={(_, value: "login" | "register") => setTab(value)}
            variant="fullWidth"
          >
            <Tab value="login" label="ログイン" />
            <Tab value="register" label="新規登録" />
          </Tabs>
          <CardContent>
            {tab === "login" ? (
              <LoginForm onAuthenticated={onAuthenticated} />
            ) : (
              <RegisterForm onAuthenticated={onAuthenticated} />
            )}
          </CardContent>
        </Card>

        <Alert severity="info">
          開発用: テナントID <strong>development</strong>、メールアドレス{" "}
          <strong>admin@example.test</strong>、パスワード{" "}
          <strong>development-password</strong>
        </Alert>
      </Stack>
    </Container>
  );
}

function LoginForm({ onAuthenticated }: AuthPageProps) {
  const [error, setError] = useState<string>();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<LoginValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: {
      tenant_slug: "development",
      email: "admin@example.test",
      password: "development-password",
    },
  });

  const submit = handleSubmit(async (values) => {
    setError(undefined);
    try {
      const response = await api.login(values);
      onAuthenticated(response.user);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  });

  return (
    <Stack component="form" spacing={2} onSubmit={submit} noValidate>
      {error !== undefined && <Alert severity="error">{error}</Alert>}
      <TextField
        label="テナントID"
        autoComplete="organization"
        error={errors.tenant_slug !== undefined}
        helperText={errors.tenant_slug?.message}
        {...register("tenant_slug")}
      />
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
      <Button type="submit" variant="contained" disabled={isSubmitting}>
        {isSubmitting ? "ログイン中..." : "ログイン"}
      </Button>
    </Stack>
  );
}

function RegisterForm({ onAuthenticated }: AuthPageProps) {
  const [error, setError] = useState<string>();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<RegisterValues>({
    resolver: zodResolver(registerSchema),
    defaultValues: {
      tenant_name: "",
      tenant_slug: "",
      display_name: "",
      email: "",
      password: "",
    },
  });

  const submit = handleSubmit(async (values) => {
    setError(undefined);
    try {
      const response = await api.register(values);
      onAuthenticated(response.user);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  });

  return (
    <Stack component="form" spacing={2} onSubmit={submit} noValidate>
      {error !== undefined && <Alert severity="error">{error}</Alert>}
      <TextField
        label="テナント名"
        autoComplete="organization"
        error={errors.tenant_name !== undefined}
        helperText={errors.tenant_name?.message}
        {...register("tenant_name")}
      />
      <TextField
        label="テナントID"
        error={errors.tenant_slug !== undefined}
        helperText={errors.tenant_slug?.message ?? "ログイン時に使用します。"}
        {...register("tenant_slug")}
      />
      <TextField
        label="管理者表示名"
        autoComplete="name"
        error={errors.display_name !== undefined}
        helperText={errors.display_name?.message}
        {...register("display_name")}
      />
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
        autoComplete="new-password"
        error={errors.password !== undefined}
        helperText={errors.password?.message ?? "12文字以上"}
        {...register("password")}
      />
      <Button type="submit" variant="contained" disabled={isSubmitting}>
        {isSubmitting ? "登録中..." : "テナントを作成"}
      </Button>
    </Stack>
  );
}

function errorMessage(cause: unknown): string {
  return cause instanceof ApiError ? cause.message : "通信に失敗しました。";
}

