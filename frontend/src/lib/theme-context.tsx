"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { isDaytime } from "./sun";

export type ThemeMode = "light" | "dark" | "auto";
export type ResolvedTheme = "light" | "dark";

interface ThemeContextValue {
  mode: ThemeMode;
  resolved: ResolvedTheme;
  setMode: (mode: ThemeMode) => void;
  /** true se la modalità "auto" sta usando alba/tramonto reali (geolocalizzazione
   * concessa) invece del fallback sulle preferenze di sistema. */
  autoUsesLocation: boolean;
}

const STORAGE_KEY = "notturni_theme_mode";
const RECHECK_INTERVAL_MS = 10 * 60 * 1000; // 10 minuti: sufficiente a cogliere alba/tramonto senza sprecare risorse

const ThemeContext = createContext<ThemeContextValue | null>(null);

function prefersDarkFallback(): ResolvedTheme {
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>("auto");
  const [resolved, setResolved] = useState<ResolvedTheme>("light");
  const [autoUsesLocation, setAutoUsesLocation] = useState(false);
  const [hydrated, setHydrated] = useState(false);

  // idratazione da localStorage: non disponibile lato server, va per forza
  // letta qui e non con uno stato iniziale "lazy"
  useEffect(() => {
    const stored = window.localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
    if (stored === "light" || stored === "dark" || stored === "auto") {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setModeState(stored);
    }
    setHydrated(true);
  }, []);

  const applyAuto = useCallback(() => {
    if (!navigator.geolocation) {
      setAutoUsesLocation(false);
      setResolved(prefersDarkFallback());
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (position) => {
        // la posizione resta solo in memoria per questo calcolo: non viene
        // mai salvata né inviata al backend
        const { latitude, longitude } = position.coords;
        setAutoUsesLocation(true);
        setResolved(isDaytime(new Date(), latitude, longitude) ? "light" : "dark");
      },
      () => {
        setAutoUsesLocation(false);
        setResolved(prefersDarkFallback());
      },
      { timeout: 8000, maximumAge: RECHECK_INTERVAL_MS }
    );
  }, []);

  useEffect(() => {
    if (!hydrated) return;

    if (mode !== "auto") {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setAutoUsesLocation(false);
      setResolved(mode);
      return;
    }

    applyAuto();
    const interval = window.setInterval(applyAuto, RECHECK_INTERVAL_MS);
    const onVisible = () => {
      if (document.visibilityState === "visible") applyAuto();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [mode, hydrated, applyAuto]);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  const setMode = useCallback((next: ThemeMode) => {
    setModeState(next);
    window.localStorage.setItem(STORAGE_KEY, next);
  }, []);

  const value = useMemo(
    () => ({ mode, resolved, setMode, autoUsesLocation }),
    [mode, resolved, setMode, autoUsesLocation]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme deve essere usato dentro ThemeProvider");
  return ctx;
}
