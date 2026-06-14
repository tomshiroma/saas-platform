import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Stack,
  Typography,
} from "@mui/material";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { ApiError, api, type CurrentUser } from "./api";

export function BillingPage({ currentUser }: { currentUser: CurrentUser }) {
  const [searchParams] = useSearchParams();
  const plans = useQuery({
    queryKey: ["billing-plans"],
    queryFn: api.billingPlans,
  });
  const status = useQuery({
    queryKey: ["billing-status"],
    queryFn: api.billingStatus,
    refetchInterval: searchParams.get("checkout") === "success" ? 3000 : false,
  });

  if (currentUser.role !== "admin") {
    return <Alert severity="warning">契約管理には管理者権限が必要です。</Alert>;
  }

  return (
    <Stack spacing={3}>
      <Box>
        <Typography component="h1" variant="h3" gutterBottom>
          契約・お支払い
        </Typography>
        <Typography color="text.secondary">
          Stripeを通じてプランと支払い方法を管理します。
        </Typography>
      </Box>
      {searchParams.get("checkout") === "success" && (
        <Alert severity="success">
          お申し込みを受け付けました。Stripeからの契約確定を確認しています。
        </Alert>
      )}
      {searchParams.get("checkout") === "cancel" && (
        <Alert severity="info">お申し込みはキャンセルされました。</Alert>
      )}
      <CurrentSubscription currentUser={currentUser} status={status} />
      <PlanList currentUser={currentUser} plans={plans} status={status} />
    </Stack>
  );
}

function CurrentSubscription({
  currentUser,
  status,
}: {
  currentUser: CurrentUser;
  status: ReturnType<typeof useQuery<Awaited<ReturnType<typeof api.billingStatus>>>>;
}) {
  const portal = useMutation({
    mutationFn: () => api.createBillingPortal(currentUser.csrf_token),
    onSuccess: ({ url }) => window.location.assign(url),
  });

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Typography component="h2" variant="h5">
            現在の契約
          </Typography>
          {status.isPending ? (
            <CircularProgress />
          ) : status.isError ? (
            <Alert severity="error">{errorMessage(status.error)}</Alert>
          ) : status.data.plan === null ? (
            <Alert severity="info">契約中の有料プランはありません。</Alert>
          ) : (
            <>
              <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                <Typography variant="h6">{status.data.plan.name}</Typography>
                <Chip label={statusLabel(status.data.status)} />
              </Stack>
              <Typography>
                {formatAmount(
                  status.data.unit_amount ?? status.data.plan.unit_amount,
                )}{" "}
                /{" "}
                {(status.data.billing_interval ??
                  status.data.plan.billing_interval) === "month"
                  ? "月"
                  : "年"}
              </Typography>
              {status.data.current_period_end !== null && (
                <Typography color="text.secondary">
                  現在の請求期間終了: {formatDate(status.data.current_period_end)}
                </Typography>
              )}
              {status.data.cancel_at_period_end && (
                <Alert severity="warning">請求期間終了時に解約予定です。</Alert>
              )}
              {portal.isError && (
                <Alert severity="error">{errorMessage(portal.error)}</Alert>
              )}
              <Button
                variant="outlined"
                onClick={() => portal.mutate()}
                disabled={portal.isPending || !status.data.stripe_configured}
                sx={{ alignSelf: "flex-start" }}
              >
                支払い方法・解約を管理
              </Button>
            </>
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}

function PlanList({
  currentUser,
  plans,
  status,
}: {
  currentUser: CurrentUser;
  plans: ReturnType<typeof useQuery<Awaited<ReturnType<typeof api.billingPlans>>>>;
  status: ReturnType<typeof useQuery<Awaited<ReturnType<typeof api.billingStatus>>>>;
}) {
  const checkout = useMutation({
    mutationFn: (planId: string) =>
      api.createCheckout(planId, currentUser.csrf_token),
    onSuccess: ({ url }) => window.location.assign(url),
  });

  return (
    <Card>
      <CardContent>
        <Stack spacing={2}>
          <Typography component="h2" variant="h5">
            プラン
          </Typography>
          {checkout.isError && (
            <Alert severity="error">{errorMessage(checkout.error)}</Alert>
          )}
          {plans.isPending || status.isPending ? (
            <CircularProgress />
          ) : plans.isError ? (
            <Alert severity="error">{errorMessage(plans.error)}</Alert>
          ) : status.isError ? (
            <Alert severity="error">{errorMessage(status.error)}</Alert>
          ) : plans.data.length === 0 ? (
            <Alert severity="info">現在申し込めるプランはありません。</Alert>
          ) : (
            <Box
              sx={{
                display: "grid",
                gap: 2,
                gridTemplateColumns: { xs: "1fr", md: "repeat(3, 1fr)" },
              }}
            >
              {plans.data.map((plan) => {
                const current = status.data.plan?.id === plan.id;
                return (
                  <Card key={plan.id} variant="outlined">
                    <CardContent>
                      <Stack spacing={2}>
                        <Typography variant="h5">{plan.name}</Typography>
                        <Typography variant="h4">
                          {formatAmount(plan.unit_amount)}
                        </Typography>
                        <Typography color="text.secondary">
                          {plan.billing_interval === "month" ? "月額" : "年額"}
                        </Typography>
                        <Typography>{plan.description}</Typography>
                        <Button
                          variant={current ? "outlined" : "contained"}
                          disabled={
                            current ||
                            checkout.isPending ||
                            !status.data.stripe_configured
                          }
                          onClick={() => checkout.mutate(plan.id)}
                        >
                          {current ? "契約中" : "Stripeで申し込む"}
                        </Button>
                      </Stack>
                    </CardContent>
                  </Card>
                );
              })}
            </Box>
          )}
          {status.data !== undefined && !status.data.stripe_configured && (
            <Alert severity="warning">
              Stripeが設定されていないため、現在は申し込みできません。
            </Alert>
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}

function statusLabel(status: string | null): string {
  const labels: Record<string, string> = {
    active: "契約中",
    trialing: "トライアル",
    past_due: "支払い遅延",
    unpaid: "未払い",
    canceled: "解約済み",
    checkout_pending: "申込処理中",
  };
  return status === null ? "未契約" : (labels[status] ?? status);
}

function formatAmount(value: number): string {
  return new Intl.NumberFormat("ja-JP", {
    style: "currency",
    currency: "JPY",
  }).format(value);
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
