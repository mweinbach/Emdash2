import { createContext, useEffect, useState, type ReactNode } from 'react';

type Theme = 'light' | 'dark' | 'system';
const LOCKED_THEME: Theme = 'system';

function getSystemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(effectiveTheme: 'light' | 'dark') {
  if (typeof document === 'undefined') return;

  const root = document.documentElement;

  if (effectiveTheme === 'dark') {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
}

interface ThemeContextType {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  effectiveTheme: 'light' | 'dark';
}

export const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme] = useState<Theme>(LOCKED_THEME);
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(getSystemTheme);

  const effectiveTheme = systemTheme;

  useEffect(() => {
    applyTheme(effectiveTheme);
  }, [effectiveTheme]);

  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => {
      setSystemTheme(getSystemTheme());
    };

    // Modern browsers
    if (mediaQuery.addEventListener) {
      mediaQuery.addEventListener('change', handler);
      return () => mediaQuery.removeEventListener('change', handler);
    }

    // Legacy browsers
    mediaQuery.addListener(handler);
    return () => mediaQuery.removeListener(handler);
  }, [theme]);

  const setTheme = () => {
    // Theme selection is intentionally locked to system.
  };

  const toggleTheme = () => {
    // Theme selection is intentionally locked to system.
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme, toggleTheme, effectiveTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}
