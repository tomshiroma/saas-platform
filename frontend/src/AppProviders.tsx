import { CssBaseline, ThemeProvider, createTheme } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useMemo } from "react";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { ColorModeProvider } from "./ColorMode";
import {
  useColorMode,
  type ColorMode,
} from "./color-mode-context";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
    },
  },
});

export function AppProviders() {
  return (
    <ColorModeProvider>
      <ThemedApplication />
    </ColorModeProvider>
  );
}

function ThemedApplication() {
  const { mode } = useColorMode();
  const theme = useMemo(() => createAppTheme(mode), [mode]);

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    </ThemeProvider>
  );
}

function createAppTheme(mode: ColorMode) {
  const dark = mode === "dark";
  return createTheme({
    palette: {
      mode,
      primary: {
        main: "#0891b2",
        dark: "#0e7490",
        light: "#cffafe",
      },
      secondary: {
        main: "#10b981",
      },
      background: {
        default: dark ? "#07111f" : "#f8fafc",
        paper: dark ? "#111c2e" : "#ffffff",
      },
      text: {
        primary: dark ? "#e6edf7" : "#0f172a",
        secondary: dark ? "#94a3b8" : "#64748b",
      },
      divider: dark ? "#263449" : "#e2e8f0",
      success: {
        main: "#059669",
        light: "#d1fae5",
        dark: "#065f46",
      },
    },
    shape: {
      borderRadius: 8,
    },
    typography: {
      fontFamily:
        '"Inter", "Noto Sans JP", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
      h3: {
        fontWeight: 700,
      },
      h4: {
        fontWeight: 700,
      },
      h5: {
        fontWeight: 700,
      },
      button: {
        fontWeight: 700,
        textTransform: "none",
      },
    },
    components: {
      MuiCssBaseline: {
        styleOverrides: {
          body: {
            minWidth: 320,
          },
          "::selection": {
            backgroundColor: "#a5f3fc",
            color: "#164e63",
          },
        },
      },
      MuiCard: {
        styleOverrides: {
          root: {
            border: `1px solid ${dark ? "#263449" : "#e2e8f0"}`,
            boxShadow: "none",
          },
        },
      },
      MuiCardContent: {
        styleOverrides: {
          root: {
            padding: 24,
            "&:last-child": {
              paddingBottom: 24,
            },
          },
        },
      },
      MuiButton: {
        defaultProps: {
          disableElevation: true,
        },
        styleOverrides: {
          root: {
            borderRadius: 8,
            paddingInline: 16,
          },
        },
      },
      MuiTextField: {
        defaultProps: {
          variant: "outlined",
        },
      },
      MuiPaper: {
        styleOverrides: {
        rounded: {
            borderRadius: 8,
          },
        },
      },
    },
  });
}
