# Gas optimisation in the deposit path

The deposit function stores the balance once. The code checks the return value
of every transfer and carries a reentrancy guard. No oracle or price check is
needed for this path.
