# No reentrancy risk remains

The previous fix adds a reentrancy guard. The implementation prevents
reentrancy because state is committed before the external call. No funds are
lost on this path.
