#[cfg(not(target_arch = "wasm32"))]
pub mod instant_std;
#[cfg(not(target_arch = "wasm32"))]
pub use instant_std::*;

#[cfg(target_arch = "wasm32")]
pub mod instant_wasm;
#[cfg(target_arch = "wasm32")]
pub use instant_wasm::*;
