# Vault close leaves fees unsettled

Severity: High

## Summary

The operator can call `Vault.close` with a stale reserve so `FeeLedger.distributeFees` reverts for every depositor.
