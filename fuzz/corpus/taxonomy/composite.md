# Reentrancy in the withdrawal path

Severity: High

## Summary

The withdrawal function transfers `USDC` to the recipient before it commits the
new balance, so a malicious recipient re-enters the vault and withdraws again.
An attacker can drain `USDC` from the vault, a permanent loss for depositors.

## Recommendation

Update the balance before the external call and add a reentrancy guard.
