/** Minuten → "h:mm" mit Vorzeichen. */
export function hm(min: number | null | undefined, signed = false): string {
  if (min === null || min === undefined) return '';
  const sign = min < 0 ? '-' : signed && min > 0 ? '+' : '';
  const a = Math.abs(min);
  return `${sign}${Math.floor(a / 60)}:${String(a % 60).padStart(2, '0')}`;
}

export function dateDe(iso: string | null | undefined): string {
  if (!iso) return '';
  const [y, m, d] = iso.split('-');
  return `${d}.${m}.${y}`;
}

export function monthLabel(ym: string): string {
  const [y, m] = ym.split('-').map(Number);
  return new Date(y, m - 1, 1).toLocaleDateString('de-AT', { month: 'long', year: 'numeric' });
}

export function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export function thisMonth(): string {
  return todayIso().slice(0, 7);
}

export function shiftMonth(ym: string, delta: number): string {
  const [y, m] = ym.split('-').map(Number);
  const d = new Date(y, m - 1 + delta, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
}

export function days(tage: number): string {
  return Number.isInteger(tage) ? String(tage) : tage.toFixed(1).replace('.', ',');
}

export const PUNCH_LABELS: Record<string, string> = {
  kommen: 'Kommen',
  gehen: 'Gehen',
  pause_start: 'Pause Beginn',
  pause_ende: 'Pause Ende'
};

export const WARNING_TEXT: Record<string, (w: any) => string> = {
  open_shift: () => 'Kommen ohne Gehen (offene Stempelung)',
  invalid_sequence: (w) => `Ungültige Stempelfolge (${PUNCH_LABELS[w.kind] ?? w.kind})`,
  missing_break: (w) => `Pause fehlt oder zu kurz (${hm(w.break_min)} bei ${hm(w.worked_min)} Arbeit)`,
  auto_break_deducted: (w) => `Pause automatisch abgezogen (${w.minutes} min)`,
  over10h: (w) => `Tagesarbeitszeit über 10 Stunden (${hm(w.worked_min)})`,
  over12h: (w) => `Tagesarbeitszeit über 12 Stunden (${hm(w.worked_min)}), § 9 AZG`,
  rest_time_short: (w) => `Ruhezeit zum Vortag unter 11 Stunden (${hm(w.rest_min)}), § 12 AZG`,
  work_on_holiday: () => 'Arbeit an einem Feiertag',
  work_on_sunday: () => 'Arbeit an einem Sonntag',
  absence_and_punches: () => 'Ganztägige Abwesenheit und Stempelungen am selben Tag',
  outside_flex_frame: (w) => `Stempelung ${w.at.slice(0, 5)} außerhalb des Gleitzeitrahmens`,
  week_over50h: (w) => `Wochenarbeitszeit über 50 Stunden (${hm(w.worked_min)})`,
  week_over60h: (w) => `Wochenarbeitszeit über 60 Stunden (${hm(w.worked_min)}), § 9 AZG`
};

export function warningText(w: any): string {
  const f = WARNING_TEXT[w.code];
  return f ? f(w) : w.code;
}

export const ABSENCE_KINDS: { art: string; label: string; self: boolean }[] = [
  { art: 'urlaub', label: 'Urlaub', self: true },
  { art: 'zeitausgleich', label: 'Zeitausgleich', self: true },
  { art: 'krank', label: 'Krankenstand', self: false },
  { art: 'arbeitsunfall', label: 'Arbeitsunfall', self: false },
  { art: 'freizeitunfall', label: 'Freizeitunfall', self: false },
  { art: 'pflegeurlaub', label: 'Pflegeurlaub', self: true },
  { art: 'absonderung', label: 'Behördliche Absonderung', self: false },
  { art: 'sonderurlaub', label: 'Sonderurlaub', self: true },
  { art: 'pers_feiertag', label: 'Persönlicher Feiertag', self: true },
  { art: 'arzt', label: 'Arztbesuch', self: true },
  { art: 'dienstreise', label: 'Dienstreise', self: true },
  { art: 'unbezahlt', label: 'Unbezahlter Urlaub', self: false }
];

export const STATUS_LABEL: Record<string, string> = {
  beantragt: 'offen',
  genehmigt: 'genehmigt',
  abgelehnt: 'abgelehnt',
  storniert: 'storniert'
};

/** Kurzbeschreibung eines Korrekturantrags. */
export function requestText(r: any): string {
  if (r.typ === 'tag') {
    const list = Array.isArray(r.stempelungen) ? r.stempelungen : [];
    return list.length ? 'Tag ändern: ' + list.map((p: any) => `${p.zeit} ${PUNCH_LABELS[p.art] ?? p.art}`).join(', ') : 'Tag ändern: alle Stempelungen entfernen';
  }
  if (r.typ === 'einfuegen') return `${PUNCH_LABELS[r.art] ?? r.art} ${r.zeit} nachtragen`;
  return 'Stempelung streichen';
}
