pub mod contract;
mod helpers;
mod state;

#[cfg(all(target_arch = "wasm32", not(feature = "library")))]
cosmwasm_std::create_entry_points!(contract);
