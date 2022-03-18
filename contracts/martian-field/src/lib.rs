#[cfg(not(feature = "library"))]
pub mod contract;
pub mod execute;
pub mod execute_callbacks;
pub mod execute_replies;
pub mod health;
pub mod helpers;
pub mod legacy;
pub mod queries;
pub mod state;

#[cfg(test)]
mod contract_tests;
