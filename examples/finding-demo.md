# Self-backed vaults switch off the surplus distribution

**Severity:** High

## Summary

`VaultEngine.open` accepts any initialised collateral type. Any user can open a vault on the
fee-exempt stable collateral (USDT-A, USDC-A) and draw USDR 1:1. The resulting debt makes
`BalanceSheet.distributeSurplus` revert with `ReserveBackingShortfall` on every call, and the
draw consumes the debt ceiling that `PegStabilityModule.sellStable` depends on, so later
deposits revert `CeilingExceeded`.

## Preconditions

- Attacker is any user; no role, no privileged call.
- No oracle or price movement is needed.
- No front-running.

## Citations

- `VaultEngine.open`
- `BalanceSheet.distributeSurplus`
- `PegStabilityModule.sellStable`
