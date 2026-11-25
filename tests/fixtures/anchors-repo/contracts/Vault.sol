// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./Base.sol";

contract Vault is Base {
    uint256 public totalAssets;
    mapping(address => uint256) internal balances;

    event Deposited(address indexed who, uint256 amount);

    function deposit(uint256 amount) external {
        balances[msg.sender] += amount;
        totalAssets += amount;
        emit Deposited(msg.sender, amount);
    }

    function compound() external {
        totalAssets = baseIncrement() + totalAssets;
    }

    function _helper(uint256 x) internal pure returns (uint256) {
        return x * 2;
    }

    function useHelper(uint256 y) external pure returns (uint256) {
        return _helper(y);
    }
}
