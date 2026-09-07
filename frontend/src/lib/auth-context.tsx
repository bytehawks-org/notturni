"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { ApiClientError, api } from "./api";
import type { CurrentUser, LoginResponse } from "./types";
import { isMfaRequired } from "./types";

interface AuthContextValue {
  user: CurrentUser | null;
  accessToken: string | null;
  loading: boolean;
  login: (email: string, password: string) => Promise<LoginResponse>;
  verifyMfa: (challenge: string, code: string) => Promise<void>;
  register: (username: string, email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
  /** Esegue una chiamata autenticata; se l'access token è scaduto (401) tenta
   * un refresh e riprova una volta sola. */
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<CurrentUser | null>(null);
  // L'access token vive solo in memoria (mai in localStorage): sopravvive
  // alla navigazione client-side ma si perde a un reload completo, motivo
  // per cui all'avvio si tenta sempre un refresh silenzioso — il refresh
  // token vero e proprio è nel cookie httpOnly impostato dal backend
  // (ROADMAP.md "Sessione in localStorage", backend/app/api/v1/auth.py).
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.auth
      .refresh()
      .then(async (session) => {
        setAccessToken(session.access_token);
        setUser(await api.auth.me(session.access_token));
      })
      .catch(() => {
        setAccessToken(null);
      })
      .finally(() => setLoading(false));
  }, []);

  const login = useCallback(async (email: string, password: string): Promise<LoginResponse> => {
    const res = await api.auth.login({ email, password });
    if (!isMfaRequired(res)) {
      setAccessToken(res.access_token);
      setUser(await api.auth.me(res.access_token));
    }
    return res;
  }, []);

  const verifyMfa = useCallback(async (challenge: string, code: string) => {
    const session = await api.auth.verifyMfa({ challenge, code });
    setAccessToken(session.access_token);
    setUser(await api.auth.me(session.access_token));
  }, []);

  const register = useCallback(async (username: string, email: string, password: string) => {
    await api.auth.register({ username, email, password });
  }, []);

  const logout = useCallback(async () => {
    try {
      await api.auth.logout();
    } catch {
      // la sessione locale va comunque ripulita anche se la revoca remota fallisce
    }
    setUser(null);
    setAccessToken(null);
  }, []);

  const refreshUser = useCallback(async () => {
    if (!accessToken) return;
    setUser(await api.auth.me(accessToken));
  }, [accessToken]);

  const authFetch = useCallback(
    async <T,>(fn: (token: string) => Promise<T>): Promise<T> => {
      if (!accessToken) throw new ApiClientError(401, "Non autenticato.");
      try {
        return await fn(accessToken);
      } catch (err) {
        if (err instanceof ApiClientError && err.status === 401) {
          const session = await api.auth.refresh();
          setAccessToken(session.access_token);
          return await fn(session.access_token);
        }
        throw err;
      }
    },
    [accessToken]
  );

  const value = useMemo(
    () => ({ user, accessToken, loading, login, verifyMfa, register, logout, refreshUser, authFetch }),
    [user, accessToken, loading, login, verifyMfa, register, logout, refreshUser, authFetch]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth deve essere usato dentro AuthProvider");
  return ctx;
}
