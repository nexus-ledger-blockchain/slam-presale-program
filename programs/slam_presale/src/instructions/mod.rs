pub mod admin;
pub mod buy_with_sol;
pub mod buy_with_stable;
pub mod claim_tokens;
pub mod enable_claim;
pub mod finalize;
pub mod initialize;
pub mod refund;

// Glob re-exports (not just the named Accounts structs) are required here:
// the #[derive(Accounts)] macro also generates a sibling `__client_accounts_*`
// module per instruction that the top-level #[program] macro in lib.rs needs
// to reach via `crate::instructions::*` — a named re-export only pulls the
// struct itself and silently breaks that macro's expansion.
pub use admin::*;
pub use buy_with_sol::*;
pub use buy_with_stable::*;
pub use claim_tokens::*;
pub use enable_claim::*;
pub use finalize::*;
pub use initialize::*;
pub use refund::*;
