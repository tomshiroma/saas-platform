import { Alert, Box, CircularProgress, Stack, Typography } from "@mui/material";
import { useEffect, useState } from "react";

type MfaQrCodeProps = {
  provisioningUri: string;
};

export function MfaQrCode({ provisioningUri }: MfaQrCodeProps) {
  const [result, setResult] = useState<{
    provisioningUri: string;
    dataUrl?: string;
    failed?: boolean;
  }>();

  useEffect(() => {
    let active = true;

    void import("qrcode")
      .then(({ default: QRCode }) =>
        QRCode.toDataURL(provisioningUri, {
          errorCorrectionLevel: "M",
          margin: 2,
          width: 240,
          color: {
            dark: "#111827",
            light: "#ffffff",
          },
        }),
      )
      .then((url) => {
        if (active) {
          setResult({ provisioningUri, dataUrl: url });
        }
      })
      .catch(() => {
        if (active) {
          setResult({ provisioningUri, failed: true });
        }
      });

    return () => {
      active = false;
    };
  }, [provisioningUri]);

  const currentResult =
    result?.provisioningUri === provisioningUri ? result : undefined;

  if (currentResult?.failed === true) {
    return (
      <Alert severity="warning">
        QRコードを生成できませんでした。セットアップキーを手動で入力してください。
      </Alert>
    );
  }

  return (
    <Stack spacing={1} sx={{ alignItems: "center" }}>
      <Typography variant="body2" color="text.secondary">
        認証アプリでQRコードを読み取ってください。
      </Typography>
      <Box
        sx={{
          width: 256,
          height: 256,
          display: "grid",
          placeItems: "center",
          bgcolor: "#fff",
          border: 1,
          borderColor: "divider",
          borderRadius: 1,
          p: 1,
        }}
      >
        {currentResult?.dataUrl === undefined ? (
          <CircularProgress size={32} aria-label="QRコードを生成中" />
        ) : (
          <Box
            component="img"
            src={currentResult.dataUrl}
            alt="多要素認証セットアップ用QRコード"
            sx={{ display: "block", width: 240, height: 240 }}
          />
        )}
      </Box>
    </Stack>
  );
}
