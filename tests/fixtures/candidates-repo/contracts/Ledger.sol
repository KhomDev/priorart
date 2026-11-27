// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Ledger {
    uint256 public totalAssets;
    mapping(address => uint256) internal balances;
    uint256 public feeBps;

    function entry(uint256 amount) external {
        middle(amount);
    }

    function middle(uint256 amount) internal {
        leaf(amount);
    }

    function leaf(uint256 amount) internal {
        totalAssets += amount;
    }

    function deposit(uint256 amount) external {
        balances[msg.sender] += amount;
        totalAssets += amount;
    }

    function alpha(uint256 amount) external pure returns (uint256) {
        return amount / 3;
    }

    function beta(uint256 amount) external pure returns (uint256) {
        return amount / 5;
    }
}
