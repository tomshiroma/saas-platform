import { IconButton, SvgIcon, Tooltip } from "@mui/material";
import {
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  ColorModeContext,
  useColorMode,
  type ColorMode,
} from "./color-mode-context";

const STORAGE_KEY = "saas-platform:color-mode:v1";

export function ColorModeProvider({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<ColorMode>(readStoredMode);
  const value = useMemo(
    () => ({
      mode,
      toggleMode: () => {
        setMode((current) => {
          const next = current === "light" ? "dark" : "light";
          window.localStorage.setItem(STORAGE_KEY, next);
          return next;
        });
      },
    }),
    [mode],
  );

  return (
    <ColorModeContext.Provider value={value}>
      {children}
    </ColorModeContext.Provider>
  );
}

export function ColorModeButton({ inverted = false }: { inverted?: boolean }) {
  const { mode, toggleMode } = useColorMode();
  const nextModeLabel = mode === "light" ? "ダークモード" : "ライトモード";

  return (
    <Tooltip title={`${nextModeLabel}に切り替え`}>
      <IconButton
        aria-label={`${nextModeLabel}に切り替え`}
        onClick={toggleMode}
        sx={
          inverted
            ? {
                color: "rgba(255,255,255,0.72)",
                "&:hover": {
                  color: "common.white",
                  bgcolor: "rgba(255,255,255,0.08)",
                },
              }
            : undefined
        }
      >
        {mode === "light" ? <DarkModeIcon /> : <LightModeIcon />}
      </IconButton>
    </Tooltip>
  );
}

function readStoredMode(): ColorMode {
  if (typeof window === "undefined") {
    return "light";
  }
  return window.localStorage.getItem(STORAGE_KEY) === "dark" ? "dark" : "light";
}

function DarkModeIcon() {
  return (
    <SvgIcon>
      <path d="M9.37 5.51A7 7 0 0 0 17.49 14 7 7 0 1 1 9.37 5.51Z" />
    </SvgIcon>
  );
}

function LightModeIcon() {
  return (
    <SvgIcon>
      <path d="M12 4.5A1.5 1.5 0 1 0 12 1a1.5 1.5 0 0 0 0 3.5Zm0 15A1.5 1.5 0 1 0 12 23a1.5 1.5 0 0 0 0-3.5ZM4.5 12A1.5 1.5 0 1 0 1 12a1.5 1.5 0 0 0 3.5 0Zm15 0a1.5 1.5 0 1 0 3.5 0 1.5 1.5 0 0 0-3.5 0ZM6.7 8.1A1.5 1.5 0 1 0 4.22 6.4 1.5 1.5 0 0 0 6.7 8.1Zm11.08 9.5a1.5 1.5 0 1 0 2.48 1.7 1.5 1.5 0 0 0-2.48-1.7Zm0-11.2a1.5 1.5 0 1 0 2.48-1.7 1.5 1.5 0 0 0-2.48 1.7ZM6.7 15.9a1.5 1.5 0 1 0-2.48 1.7 1.5 1.5 0 0 0 2.48-1.7ZM12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10Z" />
    </SvgIcon>
  );
}
