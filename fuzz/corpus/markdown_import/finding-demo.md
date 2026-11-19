# Closing a loan skips the fee settlement

**Severity:** High

## Summary

`LoanBook.close` lets any borrower close a loan without settling the fee index. The unsettled
amount stays on the books of `FeeLedger`, so every later `FeeLedger.distributeFees` call reverts
with `FeeAccountingMismatch`, and the stuck fees count against the cap that `SwapDesk.swapIn`
checks, so later swaps revert `CapExceeded`.

## Preconditions

- Attacker is any user; no role, no privileged call.
- No oracle or price movement is needed.
- No front-running.

## Citations

- `LoanBook.close`
- `FeeLedger.distributeFees`
- `SwapDesk.swapIn`
