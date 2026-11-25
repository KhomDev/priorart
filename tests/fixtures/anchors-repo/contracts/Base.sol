// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Base {
    uint256 internal baseValue;

    function baseIncrement() internal virtual returns (uint256) {
        baseValue += 1;
        return baseValue;
    }

    function peek() external pure returns (uint256) {
        return 7;
    }
}
