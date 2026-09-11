-- Saldo zum Monatsende beim Abschluss festhalten, damit spätere Berechnungen dort aufsetzen können.
ALTER TABLE month_closures ADD COLUMN saldo_ende_min INTEGER;
