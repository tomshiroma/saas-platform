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
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import {
  ApiError,
  api,
  type BillingPlan,
  type CreateBillingPlanInput,
  type PlatformAdmin,
} from "./api";

const planSchema = z.object({
  code: z
    .string()
    .min(2, "2文字以上で入力してください。")
    .max(50)
    .regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/, "英小文字、数字、ハイフンで入力してください。"),
  name: z.string().trim().min(1, "プラン名を入力してください。").max(100),
  description: z.string().trim().max(500),
  unit_amount: z.number().int().min(1).max(100_000_000),
  billing_interval: z.enum(["month", "year"]),
});

type PlanValues = z.infer<typeof planSchema>;

export function PlatformPlanManagement({ admin }: { admin: PlatformAdmin }) {
  const queryClient = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingPlan, setEditingPlan] = useState<BillingPlan>();
  const plans = useQuery({
    queryKey: ["platform-plans"],
    queryFn: api.platformPlans,
  });

  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ["platform-plans"] });
  };

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Stack direction="row" sx={{ justifyContent: "space-between" }}>
            <Box>
              <Typography component="h2" variant="h5">
                課金プラン
              </Typography>
              <Typography color="text.secondary">
                Stripe ProductとPriceへ同期されます。
              </Typography>
            </Box>
            <Button
              variant="contained"
              onClick={() => {
                setEditingPlan(undefined);
                setDialogOpen(true);
              }}
            >
              プラン追加
            </Button>
          </Stack>
          {plans.isPending ? (
            <Typography>読み込み中...</Typography>
          ) : plans.isError ? (
            <Alert severity="error">{errorMessage(plans.error)}</Alert>
          ) : plans.data.length === 0 ? (
            <Alert severity="info">課金プランはありません。</Alert>
          ) : (
            plans.data.map((plan) => (
              <Stack
                key={plan.id}
                direction={{ xs: "column", md: "row" }}
                spacing={2}
                sx={{
                  alignItems: { md: "center" },
                  border: 1,
                  borderColor: "divider",
                  borderRadius: 1,
                  p: 2,
                }}
              >
                <Box sx={{ flexGrow: 1 }}>
                  <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                    <Typography sx={{ fontWeight: 700 }}>{plan.name}</Typography>
                    <Chip
                      size="small"
                      color={plan.active ? "success" : "default"}
                      label={plan.active ? "販売中" : "停止中"}
                    />
                  </Stack>
                  <Typography color="text.secondary">
                    {formatAmount(plan.unit_amount)} /{" "}
                    {plan.billing_interval === "month" ? "月" : "年"} ・{" "}
                    {plan.code}
                  </Typography>
                  {plan.description !== "" && (
                    <Typography>{plan.description}</Typography>
                  )}
                </Box>
                <Button
                  variant="outlined"
                  onClick={() => {
                    setEditingPlan(plan);
                    setDialogOpen(true);
                  }}
                >
                  編集
                </Button>
              </Stack>
            ))
          )}
        </Stack>
      </CardContent>
      <PlanDialog
        key={editingPlan?.id ?? "new"}
        open={dialogOpen}
        admin={admin}
        plan={editingPlan}
        onClose={() => setDialogOpen(false)}
        onSaved={async () => {
          setDialogOpen(false);
          await refresh();
        }}
      />
    </Card>
  );
}

function PlanDialog({
  open,
  admin,
  plan,
  onClose,
  onSaved,
}: {
  open: boolean;
  admin: PlatformAdmin;
  plan?: BillingPlan;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [error, setError] = useState<string>();
  const [active, setActive] = useState(plan?.active ?? true);
  const {
    register,
    control,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<PlanValues>({
    resolver: zodResolver(planSchema),
    values: {
      code: plan?.code ?? "",
      name: plan?.name ?? "",
      description: plan?.description ?? "",
      unit_amount: plan?.unit_amount ?? 1000,
      billing_interval: plan?.billing_interval ?? "month",
    },
  });
  const mutation = useMutation({
    mutationFn: (values: PlanValues) => {
      const input: CreateBillingPlanInput = values;
      return plan === undefined
        ? api.createPlatformPlan(input, admin.csrf_token)
        : api.updatePlatformPlan(
            plan.id,
            {
              name: input.name,
              description: input.description,
              unit_amount: input.unit_amount,
              billing_interval: input.billing_interval,
              active,
            },
            admin.csrf_token,
          );
    },
    onSuccess: async () => {
      setError(undefined);
      reset();
      await onSaved();
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  return (
    <Dialog open={open} onClose={onClose} fullWidth maxWidth="sm">
      <Box component="form" onSubmit={handleSubmit((values) => mutation.mutate(values))}>
        <DialogTitle>{plan === undefined ? "プラン追加" : "プラン編集"}</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {error !== undefined && <Alert severity="error">{error}</Alert>}
            <TextField
              label="プランコード"
              disabled={plan !== undefined}
              error={errors.code !== undefined}
              helperText={errors.code?.message}
              {...register("code")}
            />
            <TextField
              label="プラン名"
              error={errors.name !== undefined}
              helperText={errors.name?.message}
              {...register("name")}
            />
            <TextField
              label="説明"
              multiline
              minRows={2}
              error={errors.description !== undefined}
              helperText={errors.description?.message}
              {...register("description")}
            />
            <TextField
              label="料金（円）"
              type="number"
              error={errors.unit_amount !== undefined}
              helperText={errors.unit_amount?.message}
              {...register("unit_amount", { valueAsNumber: true })}
            />
            <Controller
              control={control}
              name="billing_interval"
              render={({ field }) => (
                <FormControl>
                  <InputLabel>請求間隔</InputLabel>
                  <Select label="請求間隔" {...field}>
                    <MenuItem value="month">月額</MenuItem>
                    <MenuItem value="year">年額</MenuItem>
                  </Select>
                </FormControl>
              )}
            />
            {plan !== undefined && (
              <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                <Switch
                  checked={active}
                  onChange={(event) => setActive(event.target.checked)}
                />
                <Typography>新規販売を有効にする</Typography>
              </Stack>
            )}
            {plan !== undefined && (
              <Alert severity="info">
                料金または請求間隔を変更すると新しいStripe Priceが作成されます。
                既存契約の価格は自動変更されません。
              </Alert>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>キャンセル</Button>
          <Button type="submit" variant="contained" disabled={mutation.isPending}>
            保存
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
}

function formatAmount(value: number): string {
  return new Intl.NumberFormat("ja-JP", {
    style: "currency",
    currency: "JPY",
  }).format(value);
}

function errorMessage(cause: unknown): string {
  return cause instanceof ApiError ? cause.message : "通信に失敗しました。";
}
