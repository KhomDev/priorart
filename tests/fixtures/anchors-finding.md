# Vault deposit writes totalAssets twice

`Vault.deposit` in `contracts/Vault.sol` updates `totalAssets` before the call.

https://github.com/example/anchor-fixture/blob/main/contracts/Vault.sol#L12-L16

`doesNotExist()` is also referenced.
