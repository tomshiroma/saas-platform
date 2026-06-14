import { createContext, useContext } from "react";

export type ColorMode = "light" | "dark";

export type ColorModeContextValue = {
  mode: ColorMode;
  toggleMode: () => void;
};

export const ColorModeContext = createContext<ColorModeContextValue>({
  mode: "light",
  toggleMode: () => undefined,
});

export function useColorMode() {
  return useContext(ColorModeContext);
}
