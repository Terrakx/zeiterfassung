import { writable } from 'svelte/store';
import { api } from './api';

export interface User {
  id: number;
  personalnr: string;
  vorname: string;
  nachname: string;
  username: string;
  rolle: 'admin' | 'mitarbeiter';
  hat_pin: boolean;
  stempelt: boolean;
}

export interface Branding {
  firmenname: string;
  logo_data_url: string | null;
  primaerfarbe: string;
  fusszeile: string;
  terminal_dunkel: boolean;
  urlaub_halbe_tage: boolean;
  urlaub_stunden: boolean;
  urlaub_hinweis: string;
}

export const user = writable<User | null | undefined>(undefined);
export const branding = writable<Branding>({
  firmenname: 'Zeiterfassung',
  logo_data_url: null,
  primaerfarbe: '#2f6f8f',
  fusszeile: '',
  terminal_dunkel: true,
  urlaub_halbe_tage: false,
  urlaub_stunden: false,
  urlaub_hinweis: ''
});

export async function loadBranding() {
  try {
    const b = await api.get<Branding>('/settings/public');
    branding.set(b);
    applyTheme(b);
  } catch {
    /* Server nicht erreichbar, Standard bleibt */
  }
}

export function applyTheme(b: Branding) {
  const root = document.documentElement;
  root.style.setProperty('--primary', b.primaerfarbe || '#2f6f8f');
  root.style.setProperty('--primary-dark', darken(b.primaerfarbe || '#2f6f8f'));
  document.title = b.firmenname || 'Zeiterfassung';
}

function darken(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return '#22546d';
  const n = parseInt(m[1], 16);
  const f = (c: number) => Math.max(0, Math.round(c * 0.78));
  const r = f((n >> 16) & 255), g = f((n >> 8) & 255), b = f(n & 255);
  return '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');
}

export async function loadUser() {
  try {
    user.set(await api.get<User>('/auth/me'));
  } catch {
    user.set(null);
  }
}
