import { zodResolver } from "@hookform/resolvers/zod";
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import {
  ApiError,
  api,
  type CreateUserInput,
  type CurrentUser,
  type TenantUser,
} from "./api";
import { MfaQrCode } from "./MfaQrCode";

const tenantSchema = z.object({
  name: z.string().trim().min(1, "テナント名を入力してください。").max(100),
});

const userSchema = z.object({
  email: z.email("メールアドレスを入力してください。"),
  display_name: z.string().trim().min(1, "表示名を入力してください。").max(100),
  role: z.enum(["admin", "member"]),
  password: z.string().min(12, "12文字以上で入力してください。").max(128),
});

type TenantValues = z.infer<typeof tenantSchema>;

type TenantPageProps = {
  currentUser: CurrentUser;
  onCurrentUserChanged: (user: CurrentUser) => void;
};

export function TenantPage({
  currentUser,
  onCurrentUserChanged,
}: TenantPageProps) {
  if (currentUser.role !== "admin") {
    return (
      <Alert severity="warning">
        テナント管理には管理者権限が必要です。
      </Alert>
    );
  }

  return (
    <Stack spacing={3}>
      <TenantSettings
        currentUser={currentUser}
        onCurrentUserChanged={onCurrentUserChanged}
      />
      <MfaSettings
        currentUser={currentUser}
        onCurrentUserChanged={onCurrentUserChanged}
      />
      <UserManagement currentUser={currentUser} />
    </Stack>
  );
}

function MfaSettings({
  currentUser,
  onCurrentUserChanged,
}: TenantPageProps) {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [currentCode, setCurrentCode] = useState("");
  const [setupCode, setSetupCode] = useState("");
  const [flow, setFlow] = useState<Awaited<ReturnType<typeof api.resetMfa>>>();
  const [completed, setCompleted] = useState<Awaited<
    ReturnType<typeof api.confirmMfaSetup>
  >>();
  const [error, setError] = useState<string>();
  const reset = useMutation({
    mutationFn: () =>
      api.resetMfa(password, currentCode, currentUser.csrf_token),
    onSuccess: (response) => {
      setError(undefined);
      setFlow(response);
    },
    onError: (cause) => setError(errorMessage(cause)),
  });
  const confirm = useMutation({
    mutationFn: () =>
      api.confirmMfaSetup(flow?.challenge_token ?? "", setupCode),
    onSuccess: (response) => {
      setError(undefined);
      setCompleted(response);
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const close = () => {
    setOpen(false);
    setPassword("");
    setCurrentCode("");
    setSetupCode("");
    setFlow(undefined);
    setCompleted(undefined);
    setError(undefined);
  };

  const finish = () => {
    if (completed?.user !== undefined) {
      onCurrentUserChanged(completed.user);
    }
    close();
  };

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Box>
            <Typography component="h2" variant="h5">
              多要素認証
            </Typography>
            <Typography color="text.secondary">
              TOTP認証は有効です。認証アプリを変更する場合は再設定してください。
            </Typography>
          </Box>
          <Button
            variant="outlined"
            onClick={() => setOpen(true)}
            sx={{ alignSelf: "flex-start" }}
          >
            MFAを再設定
          </Button>
        </Stack>
      </CardContent>

      <Dialog open={open} onClose={flow === undefined ? close : undefined} fullWidth maxWidth="sm">
        <DialogTitle>MFAを再設定</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {error !== undefined && <Alert severity="error">{error}</Alert>}
            {completed?.user !== undefined ? (
              <>
                <Alert severity="warning">
                  新しいリカバリーコードを安全な場所へ保存してください。以前のコードは無効です。
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
                  {(completed.recovery_codes ?? []).join("\n")}
                </Box>
              </>
            ) : flow?.status === "mfa_setup_required" ? (
              <>
                <Typography color="text.secondary">
                  認証アプリへ新しいセットアップキーを登録してください。
                </Typography>
                {flow.provisioning_uri !== undefined && (
                  <MfaQrCode provisioningUri={flow.provisioning_uri} />
                )}
                <Alert severity="info">
                  セットアップキー: <strong>{flow.secret}</strong>
                </Alert>
                <TextField
                  label="新しい6桁の認証コード"
                  value={setupCode}
                  onChange={(event) => setSetupCode(event.target.value)}
                  autoComplete="one-time-code"
                  slotProps={{ htmlInput: { inputMode: "numeric" } }}
                />
              </>
            ) : (
              <>
                <Alert severity="warning">
                  再設定すると、現在のセッション、既存のセットアップキー、リカバリーコードは無効になります。
                </Alert>
                <TextField
                  label="現在のパスワード"
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  autoComplete="current-password"
                />
                <TextField
                  label="現在の認証コードまたはリカバリーコード"
                  value={currentCode}
                  onChange={(event) => setCurrentCode(event.target.value)}
                  autoComplete="one-time-code"
                />
              </>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          {completed?.user !== undefined ? (
            <Button variant="contained" onClick={finish}>
              保存しました
            </Button>
          ) : flow?.status === "mfa_setup_required" ? (
            <Button
              variant="contained"
              onClick={() => confirm.mutate()}
              disabled={confirm.isPending || setupCode.length !== 6}
            >
              {confirm.isPending ? "確認中..." : "新しいMFAを有効にする"}
            </Button>
          ) : (
            <>
              <Button onClick={close}>キャンセル</Button>
              <Button
                color="error"
                variant="contained"
                onClick={() => reset.mutate()}
                disabled={
                  reset.isPending ||
                  password.length < 12 ||
                  currentCode.trim().length < 6
                }
              >
                {reset.isPending ? "確認中..." : "再設定を開始"}
              </Button>
            </>
          )}
        </DialogActions>
      </Dialog>
    </Card>
  );
}

function TenantSettings({
  currentUser,
  onCurrentUserChanged,
}: TenantPageProps) {
  const [message, setMessage] = useState<string>();
  const [error, setError] = useState<string>();
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<TenantValues>({
    resolver: zodResolver(tenantSchema),
    values: { name: currentUser.tenant_name },
  });
  const mutation = useMutation({
    mutationFn: (values: TenantValues) =>
      api.updateTenant(values.name, currentUser.csrf_token),
    onSuccess: (tenant) => {
      setError(undefined);
      setMessage("テナント名を更新しました。");
      onCurrentUserChanged({ ...currentUser, tenant_name: tenant.name });
    },
    onError: (cause) => {
      setMessage(undefined);
      setError(errorMessage(cause));
    },
  });

  return (
    <Card>
      <CardContent>
        <Stack
          component="form"
          spacing={2}
          onSubmit={handleSubmit((values) => mutation.mutate(values))}
        >
          <Box>
            <Typography component="h2" variant="h5">
              テナント設定
            </Typography>
            <Typography color="text.secondary">
              テナントID: {currentUser.tenant_slug}
            </Typography>
          </Box>
          {message !== undefined && <Alert severity="success">{message}</Alert>}
          {error !== undefined && <Alert severity="error">{error}</Alert>}
          <TextField
            label="テナント名"
            error={errors.name !== undefined}
            helperText={errors.name?.message}
            {...register("name")}
          />
          <Button
            type="submit"
            variant="contained"
            disabled={mutation.isPending}
            sx={{ alignSelf: "flex-start" }}
          >
            保存
          </Button>
        </Stack>
      </CardContent>
    </Card>
  );
}

function UserManagement({ currentUser }: { currentUser: CurrentUser }) {
  const queryClient = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const users = useQuery({
    queryKey: ["tenant-users"],
    queryFn: api.users,
  });

  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["tenant-users"] });
  };

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Stack direction="row" sx={{ justifyContent: "space-between" }}>
            <Typography component="h2" variant="h5">
              メンバー管理
            </Typography>
            <Button variant="contained" onClick={() => setDialogOpen(true)}>
              メンバー追加
            </Button>
          </Stack>

          {users.isPending ? (
            <Typography>読み込み中...</Typography>
          ) : users.isError ? (
            <Alert severity="error">{errorMessage(users.error)}</Alert>
          ) : users.data.length === 0 ? (
            <Alert severity="info">メンバーはいません。</Alert>
          ) : (
            <Stack spacing={2}>
              {users.data.map((user) => (
                <UserRow
                  key={user.id}
                  user={user}
                  currentUser={currentUser}
                  onChanged={refresh}
                />
              ))}
            </Stack>
          )}
        </Stack>
      </CardContent>

      <CreateUserDialog
        open={dialogOpen}
        currentUser={currentUser}
        onClose={() => setDialogOpen(false)}
        onCreated={async () => {
          setDialogOpen(false);
          await refresh();
        }}
      />
    </Card>
  );
}

function UserRow({
  user,
  currentUser,
  onChanged,
}: {
  user: TenantUser;
  currentUser: CurrentUser;
  onChanged: () => Promise<void>;
}) {
  const [displayName, setDisplayName] = useState(user.display_name);
  const [role, setRole] = useState(user.role);
  const [active, setActive] = useState(user.active);
  const [error, setError] = useState<string>();
  const update = useMutation({
    mutationFn: () =>
      api.updateUser(
        user.id,
        { display_name: displayName, role, active },
        currentUser.csrf_token,
      ),
    onSuccess: onChanged,
    onError: (cause) => setError(errorMessage(cause)),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteUser(user.id, currentUser.csrf_token),
    onSuccess: onChanged,
    onError: (cause) => setError(errorMessage(cause)),
  });

  return (
    <Box sx={{ border: 1, borderColor: "divider", borderRadius: 1, p: 2 }}>
      <Stack spacing={2}>
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          <Typography sx={{ fontWeight: 600 }}>{user.email}</Typography>
          {user.id === currentUser.id && <Chip size="small" label="自分" />}
        </Stack>
        {error !== undefined && <Alert severity="error">{error}</Alert>}
        <TextField
          label="表示名"
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
        />
        <FormControl>
          <InputLabel>権限</InputLabel>
          <Select
            label="権限"
            value={role}
            onChange={(event) => setRole(event.target.value as TenantUser["role"])}
          >
            <MenuItem value="admin">管理者</MenuItem>
            <MenuItem value="member">一般</MenuItem>
          </Select>
        </FormControl>
        <FormControlLabel
          control={
            <Switch
              checked={active}
              onChange={(event) => setActive(event.target.checked)}
            />
          }
          label="有効"
        />
        <Stack direction="row" spacing={1}>
          <Button
            variant="contained"
            onClick={() => update.mutate()}
            disabled={update.isPending}
          >
            更新
          </Button>
          <Button
            color="error"
            onClick={() => remove.mutate()}
            disabled={remove.isPending || user.id === currentUser.id}
          >
            削除
          </Button>
        </Stack>
      </Stack>
    </Box>
  );
}

function CreateUserDialog({
  open,
  currentUser,
  onClose,
  onCreated,
}: {
  open: boolean;
  currentUser: CurrentUser;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [error, setError] = useState<string>();
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<CreateUserInput>({
    resolver: zodResolver(userSchema),
    defaultValues: {
      email: "",
      display_name: "",
      role: "member",
      password: "",
    },
  });
  const mutation = useMutation({
    mutationFn: (values: CreateUserInput) =>
      api.createUser(values, currentUser.csrf_token),
    onSuccess: async () => {
      reset();
      setError(undefined);
      await onCreated();
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  return (
    <Dialog open={open} onClose={onClose} fullWidth maxWidth="sm">
      <Box component="form" onSubmit={handleSubmit((values) => mutation.mutate(values))}>
        <DialogTitle>メンバー追加</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {error !== undefined && <Alert severity="error">{error}</Alert>}
            <TextField
              label="メールアドレス"
              type="email"
              error={errors.email !== undefined}
              helperText={errors.email?.message}
              {...register("email")}
            />
            <TextField
              label="表示名"
              error={errors.display_name !== undefined}
              helperText={errors.display_name?.message}
              {...register("display_name")}
            />
            <FormControl>
              <InputLabel>権限</InputLabel>
              <Select label="権限" defaultValue="member" {...register("role")}>
                <MenuItem value="admin">管理者</MenuItem>
                <MenuItem value="member">一般</MenuItem>
              </Select>
            </FormControl>
            <TextField
              label="初期パスワード"
              type="password"
              error={errors.password !== undefined}
              helperText={errors.password?.message}
              {...register("password")}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>キャンセル</Button>
          <Button type="submit" variant="contained" disabled={mutation.isPending}>
            追加
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
}

function errorMessage(cause: unknown): string {
  return cause instanceof ApiError ? cause.message : "通信に失敗しました。";
}
