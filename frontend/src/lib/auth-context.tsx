"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { ApiClientError, api } from "./api";
import type { CurrentUser, LoginResponse } from "./types";
import { isMfaRequired } from "./types";

interface StoredSession {
  accessToken: string;
  refreshToken: string;
}

const STORAGE_KEY = "notturni_auth";

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

function readStorage(): StoredSession | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<StoredSession>;
    if (parsed.accessToken && parsed.refreshToken) return parsed as StoredSession;
  } catch {
    // ignorato: storage corrotto, si comporta come sessione assente
  }
  return null;
}

function writeStorage(session: StoredSession | null) {
  if (session) {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  } else {
    window.localStorage.removeItem(STORAGE_KEY);
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<CurrentUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [refreshToken, setRefreshToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // idratazione iniziale da localStorage: non disponibile lato server,
    // quindi va per forza fatta qui e non con uno stato iniziale "lazy"
    const stored = readStorage();
    if (!stored) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setLoading(false);
      return;
    }
    setAccessToken(stored.accessToken);
    setRefreshToken(stored.refreshToken);

    api.auth
      .me(stored.accessToken)
      .then(setUser)
      .catch(async () => {
        try {
          const session = await api.auth.refresh(stored.refreshToken);
          setAccessToken(session.access_token);
          setRefreshToken(session.refresh_token);
          writeStorage({ accessToken: session.access_token, refreshToken: session.refresh_token });
          setUser(await api.auth.me(session.access_token));
        } catch {
          writeStorage(null);
          setAccessToken(null);
          setRefreshToken(null);
        }
      })
      .finally(() => setLoading(false));
  }, []);

  const applySession = useCallback((accessTok: string, refreshTok: string) => {
    setAccessToken(accessTok);
    setRefreshToken(refreshTok);
    writeStorage({ accessToken: accessTok, refreshToken: refreshTok });
  }, []);

  const login = useCallback(
    async (email: string, password: string): Promise<LoginResponse> => {
      const res = await api.auth.login({ email, password });
      if (!isMfaRequired(res)) {
        applySession(res.access_token, res.refresh_token);
        setUser(await api.auth.me(res.access_token));
      }
      return res;
    },
    [applySession]
  );

  const verifyMfa = useCallback(
    async (challenge: string, code: string) => {
      const session = await api.auth.verifyMfa({ challenge, code });
      applySession(session.access_token, session.refresh_token);
      setUser(await api.auth.me(session.access_token));
    },
    [applySession]
  );

  const register = useCallback(async (username: string, email: string, password: string) => {
    await api.auth.register({ username, email, password });
  }, []);

  const logout = useCallback(async () => {
    if (refreshToken) {
      try {
        await api.auth.logout(refreshToken);
      } catch {
        // la sessione locale va comunque ripulita anche se la revoca remota fallisce
      }
    }
    writeStorage(null);
    setUser(null);
    setAccessToken(null);
    setRefreshToken(null);
  }, [refreshToken]);

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
        if (err instanceof ApiClientError && err.status === 401 && refreshToken) {
          const session = await api.auth.refresh(refreshToken);
          applySession(session.access_token, session.refresh_token);
          return await fn(session.access_token);
        }
        throw err;
      }
    },
    [accessToken, refreshToken, applySession]
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
