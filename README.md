# cash — трекер расходов

Категории в X-Tag, суммы в целых копейках (без float), сводки по дням/месяцам.
Запись — атомарный `.eml` (eml-tag), сводки — header-scan по X-Timestamp диапазонам.

```bash
cargo build --release

cash init ~/.cash
cash add ~/.cash 1299.99 --tag electronics --note PSVita
cash add ~/.cash 450.00 --tag food
cash day ~/.cash 2026-08-12
cash month ~/.cash 2026-08
cash tag ~/.cash food
cash report ~/.cash
```

## Свойства

- Суммы в центах/копейках (`parse_amount("12.3") -> 1230`), до 2 знаков
- Месячные границы по proleptic Gregorian (високосные годы учтены)
- Corruption-proof: одна битая запись не рушит сводки
- Построен на [emlbox](https://github.com/kostyk348/mime-os) (MIME-OS)

## Тесты

```bash
cargo test
```