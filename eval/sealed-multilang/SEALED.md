# Final preregistration

Committed after the units and before any tool is run. Nothing is added to
this corpus after this commit.

| | |
|---|---|
| Salt commit | `0bf85079b693da95beb915fa9a5a06797d97b385` |
| Units commit | `4551ecdf5d02e181c4c11787caa9eead86656263` |
| `units.json` sha256 | `3df2cdf584f5cb8344e098c6488b0b91ae4493e1d35a456cb7452adc41b87205` |
| Counted units | 629 |
| Fetch check | `python eval/sealed-multilang/fetch.py --check` ok, 625 files |
| priorart | not run |
| Reader labels | pending |

Split rule, unchanged since the salt commit:

```text
bucket = int(sha256(salt + "|" + unit_id).hexdigest()[:8], 16) % 100
0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL
```

Counts: X 614, J 5, A 10, N 0, AJ 0. Solidity 614, Solana 15, Move 0.
DEV 233, V1 146, V2 99, FINAL 151.
