# Internal review

## M-03 Fee index rounding

The consistency check in `FeeLedger.distributeFees` reverts with `FeeAccountingMismatch`
when the fee index rounds up past the collected balance. The cause is rounding in the index
update, not loan closure; the fix rounds the index down. Severity: Medium.
