import { zodResolver } from "@hookform/resolvers/zod";
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Container,
  Divider,
  Stack,
  Tab,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import { useState, type FormEvent } from "react";
import { useForm } from "react-hook-form";
import { useNavigate, useSearchParams } from "react-router-dom";
import { z } from "zod";
import {
  ApiError,
  api,
  type AuthFlowResponse,
  type CurrentUser,
} from "./api";
import { MfaQrCode } from "./MfaQrCode";

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

const passwordResetRequestSchema = loginSchema.pick({
  tenant_slug: true,
  email: true,
});

const passwordResetConfirmSchema = z
  .object({
    password: z.string().min(12, "12文字以上で入力してください。").max(128),
    password_confirmation: z.string(),
  })
  .refine((values) => values.password === values.password_confirmation, {
    message: "確認用パスワードが一致しません。",
    path: ["password_confirmation"],
  });

type LoginValues = z.infer<typeof loginSchema>;
type RegisterValues = z.infer<typeof registerSchema>;
type PasswordResetRequestValues = z.infer<
  typeof passwordResetRequestSchema
>;
type PasswordResetConfirmValues = z.infer<
  typeof passwordResetConfirmSchema
>;

type AuthPageProps = {
  onAuthenticated: (user: CurrentUser) => void;
};

export function AuthPage({ onAuthenticated }: AuthPageProps) {
  const [searchParams] = useSearchParams();
  const resetToken = searchParams.get("token");
  const [view, setView] = useState<"login" | "register" | "forgot">(
    "login",
  );
  const [mfaFlow, setMfaFlow] = useState<AuthFlowResponse>();
  const [recoveryResult, setRecoveryResult] = useState<AuthFlowResponse>();
  const showResetForm = resetToken !== null;
  const handleAuthFlow = (response: AuthFlowResponse) => {
    if (response.status === "authenticated" && response.user !== undefined) {
      if ((response.recovery_codes?.length ?? 0) > 0) {
        setRecoveryResult(response);
      } else {
        onAuthenticated(response.user);
      }
      return;
    }
    setMfaFlow(response);
  };

  return (
    <Container component="main" maxWidth="sm" sx={{ py: 8 }}>
      <Stack spacing={3}>
        <Box>
          <Typography component="h1" variant="h3" gutterBottom>
            SaaS Platform
          </Typography>
          <Typography color="text.secondary">
            {showResetForm
              ? "新しいパスワードを設定してください。"
              : "テナントIDを指定してログインしてください。"}
          </Typography>
        </Box>

        <Card>
          {!showResetForm && view !== "forgot" && (
            <Tabs
              value={view}
              onChange={(_, value: "login" | "register") => setView(value)}
              variant="fullWidth"
            >
              <Tab value="login" label="ログイン" />
              <Tab value="register" label="新規登録" />
            </Tabs>
          )}
          <CardContent>
            {recoveryResult?.user !== undefined ? (
              <RecoveryCodes
                codes={recoveryResult.recovery_codes ?? []}
                onContinue={() => onAuthenticated(recoveryResult.user!)}
              />
            ) : mfaFlow !== undefined ? (
              <MfaChallengeForm
                flow={mfaFlow}
                onCompleted={handleAuthFlow}
                onBack={() => setMfaFlow(undefined)}
              />
            ) : showResetForm ? (
              <PasswordResetConfirmForm token={resetToken} />
            ) : view === "login" ? (
              <LoginForm
                onAuthFlow={handleAuthFlow}
                onForgotPassword={() => setView("forgot")}
              />
            ) : view === "register" ? (
              <RegisterForm onAuthFlow={handleAuthFlow} />
            ) : (
              <PasswordResetRequestForm
                onBack={() => setView("login")}
              />
            )}
          </CardContent>
        </Card>

        {!showResetForm && (
          <Alert severity="info">
            開発用: テナントID <strong>development</strong>、メールアドレス{" "}
            <strong>admin@example.test</strong>、パスワード{" "}
            <strong>development-password</strong>
          </Alert>
        )}
      </Stack>
    </Container>
  );
}

function LoginForm({
  onAuthFlow,
  onForgotPassword,
}: {
  onAuthFlow: (response: AuthFlowResponse) => void;
  onForgotPassword: () => void;
}) {
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
      onAuthFlow(response);
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
      <Divider />
      <Button type="button" onClick={onForgotPassword}>
        パスワードをお忘れの方
      </Button>
    </Stack>
  );
}

function PasswordResetRequestForm({ onBack }: { onBack: () => void }) {
  const [error, setError] = useState<string>();
  const [message, setMessage] = useState<string>();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<PasswordResetRequestValues>({
    resolver: zodResolver(passwordResetRequestSchema),
    defaultValues: {
      tenant_slug: "development",
      email: "admin@example.test",
    },
  });

  const submit = handleSubmit(async (values) => {
    setError(undefined);
    setMessage(undefined);
    try {
      const response = await api.requestPasswordReset(values);
      setMessage(response.message);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  });

  return (
    <Stack component="form" spacing={2} onSubmit={submit} noValidate>
      <Typography component="h2" variant="h5">
        パスワード再設定
      </Typography>
      <Typography color="text.secondary">
        登録済みのテナントIDとメールアドレスを入力してください。
      </Typography>
      {error !== undefined && <Alert severity="error">{error}</Alert>}
      {message !== undefined && <Alert severity="success">{message}</Alert>}
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
      <Button type="submit" variant="contained" disabled={isSubmitting}>
        {isSubmitting ? "送信中..." : "再設定メールを送信"}
      </Button>
      <Button type="button" onClick={onBack}>
        ログインへ戻る
      </Button>
    </Stack>
  );
}

function PasswordResetConfirmForm({ token }: { token: string }) {
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const [completed, setCompleted] = useState(false);
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<PasswordResetConfirmValues>({
    resolver: zodResolver(passwordResetConfirmSchema),
    defaultValues: {
      password: "",
      password_confirmation: "",
    },
  });

  const submit = handleSubmit(async (values) => {
    setError(undefined);
    try {
      await api.confirmPasswordReset({
        token,
        password: values.password,
      });
      setCompleted(true);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  });

  if (completed) {
    return (
      <Stack spacing={2}>
        <Alert severity="success">
          パスワードを変更しました。新しいパスワードでログインしてください。
        </Alert>
        <Button variant="contained" onClick={() => navigate("/", { replace: true })}>
          ログインへ進む
        </Button>
      </Stack>
    );
  }

  return (
    <Stack component="form" spacing={2} onSubmit={submit} noValidate>
      {error !== undefined && <Alert severity="error">{error}</Alert>}
      <TextField
        label="新しいパスワード"
        type="password"
        autoComplete="new-password"
        error={errors.password !== undefined}
        helperText={errors.password?.message ?? "12～128文字"}
        {...register("password")}
      />
      <TextField
        label="新しいパスワード（確認）"
        type="password"
        autoComplete="new-password"
        error={errors.password_confirmation !== undefined}
        helperText={errors.password_confirmation?.message}
        {...register("password_confirmation")}
      />
      <Button type="submit" variant="contained" disabled={isSubmitting}>
        {isSubmitting ? "変更中..." : "パスワードを変更"}
      </Button>
    </Stack>
  );
}

function RegisterForm({
  onAuthFlow,
}: {
  onAuthFlow: (response: AuthFlowResponse) => void;
}) {
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
      onAuthFlow(response);
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

function MfaChallengeForm({
  flow,
  onCompleted,
  onBack,
}: {
  flow: AuthFlowResponse;
  onCompleted: (response: AuthFlowResponse) => void;
  onBack: () => void;
}) {
  const [code, setCode] = useState("");
  const [error, setError] = useState<string>();
  const [submitting, setSubmitting] = useState(false);
  const setup = flow.status === "mfa_setup_required";

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (flow.challenge_token === undefined) {
      return;
    }
    setSubmitting(true);
    setError(undefined);
    try {
      const response = setup
        ? await api.confirmMfaSetup(flow.challenge_token, code)
        : await api.verifyMfa(flow.challenge_token, code);
      onCompleted(response);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Stack component="form" spacing={2} onSubmit={submit}>
      <Typography component="h2" variant="h5">
        {setup ? "多要素認証の設定" : "多要素認証"}
      </Typography>
      <Typography color="text.secondary">
        {setup
          ? "認証アプリへシークレットを登録し、表示された6桁のコードを入力してください。"
          : "認証アプリの6桁コード、または未使用のリカバリーコードを入力してください。"}
      </Typography>
      {error !== undefined && <Alert severity="error">{error}</Alert>}
      {setup && flow.secret !== undefined && (
        <>
          {flow.provisioning_uri !== undefined && (
            <MfaQrCode provisioningUri={flow.provisioning_uri} />
          )}
          <Alert severity="info">
            セットアップキー: <strong>{flow.secret}</strong>
          </Alert>
        </>
      )}
      <TextField
        label={setup ? "6桁の認証コード" : "認証コードまたはリカバリーコード"}
        value={code}
        onChange={(event) => setCode(event.target.value)}
        autoComplete="one-time-code"
        slotProps={{ htmlInput: { inputMode: setup ? "numeric" : "text" } }}
        required
      />
      <Button type="submit" variant="contained" disabled={submitting}>
        {submitting ? "確認中..." : setup ? "MFAを有効にする" : "確認してログイン"}
      </Button>
      <Button type="button" onClick={onBack}>
        ログインへ戻る
      </Button>
    </Stack>
  );
}

function RecoveryCodes({
  codes,
  onContinue,
}: {
  codes: string[];
  onContinue: () => void;
}) {
  return (
    <Stack spacing={2}>
      <Typography component="h2" variant="h5">
        リカバリーコードを保存
      </Typography>
      <Alert severity="warning">
        認証アプリを利用できない場合に必要です。各コードは1回だけ使用できます。
      </Alert>
      <Box
        component="pre"
        sx={{
          m: 0,
          p: 2,
          border: 1,
          borderColor: "divider",
          borderRadius: 1,
          fontFamily: "monospace",
          whiteSpace: "pre-wrap",
        }}
      >
        {codes.join("\n")}
      </Box>
      <Button variant="contained" onClick={onContinue}>
        保存しました
      </Button>
    </Stack>
  );
}

function errorMessage(cause: unknown): string {
  return cause instanceof ApiError ? cause.message : "通信に失敗しました。";
}
