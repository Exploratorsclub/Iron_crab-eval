//! Invariante A.37: try_parse_pool_static ohne getTokenAccountsByOwner
//!
//! `try_parse_pool_static_from_market_account_inner` darf KEINE RPC-Index-Calls
//! (find_any_token_account_for_owner_and_mint, find_token_account_by_owner_and_mint,
//! get_token_accounts_by_owner) verwenden. Pool-ATAs muessen deterministisch
//! abgeleitet werden.

/// A.37: try_parse_pool_static_from_market_account_inner leitet ATAs ab ohne RPC-Index-Calls
#[test]
fn test_a37_no_get_token_accounts_by_owner_in_parse_pool_static() {
    let src = std::fs::read_to_string("../Iron_crab/src/solana/dex/pumpfun_amm.rs")
        .expect("Cannot read pumpfun_amm.rs — is the Iron_crab sibling directory present?");

    let fn_start = src
        .find("fn try_parse_pool_static_from_market_account_inner")
        .expect("Function try_parse_pool_static_from_market_account_inner not found in pumpfun_amm.rs");

    let fn_body = &src[fn_start..];
    let fn_end = fn_body[100..]
        .find("\nasync fn ")
        .or_else(|| fn_body[100..].find("\nfn "))
        .or_else(|| fn_body[100..].find("\npub async fn "))
        .or_else(|| fn_body[100..].find("\npub fn "))
        .map(|pos| pos + 100)
        .unwrap_or(fn_body.len());
    let fn_text = &fn_body[..fn_end];

    assert!(
        !fn_text.contains("find_any_token_account_for_owner_and_mint"),
        "A.37 VIOLATED: try_parse_pool_static_from_market_account_inner calls \
         find_any_token_account_for_owner_and_mint — must derive ATAs deterministically"
    );
    assert!(
        !fn_text.contains("find_token_account_by_owner_and_mint"),
        "A.37 VIOLATED: try_parse_pool_static_from_market_account_inner calls \
         find_token_account_by_owner_and_mint — must derive ATAs deterministically"
    );
    assert!(
        !fn_text.contains("get_token_accounts_by_owner"),
        "A.37 VIOLATED: try_parse_pool_static_from_market_account_inner calls \
         get_token_accounts_by_owner — must derive ATAs deterministically"
    );
}
