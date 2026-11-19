# Rounding drift in share accounting

Severity: Low

## Summary

Any user can deposit and withdraw in one atomic transaction to shift the rounding in
`sharePrice` by one wei per call.

## Impact

A one wei accounting drift accrues to the attacker. No funds are stolen in a single
call and the drift needs many transactions.
