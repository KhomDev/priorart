# Internal review - August 2026

## H-02 Reserve backing check

The backing check in `BalanceSheet.distributeSurplus` reverts with `ReserveBackingShortfall`
when the stable reserve is smaller than the summed debt of fee-exempt collateral types. The check
assumes every such debt has a reserve counterpart from `PegStabilityModule.sellStable`.

The review's test drove the shortfall only through direct ledger writes, never through
`VaultEngine.open` on a fee-exempt collateral type. Severity: High. Reported 2026-08-14.
