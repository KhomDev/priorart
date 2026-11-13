# Fee change can be blocked by the operator

Severity: High

## Summary

The `admin` can call `setFeeBps` in `src/Vault.sol` with a value above the
maximum, which makes every `distributeFees` call revert.

## Impact

A privileged operator can freeze distributions. This is a griefing path that requires
the operator role.
