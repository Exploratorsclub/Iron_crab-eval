//! Invarianten: Arbitrage Engine (INVARIANTS.md A.14–A.17)
//!
//! - Edge-Aggregation wählt höheren Output (aggregate_best_edges)
//! - Cycle-Ranking sortiert nach Profit (rank_triangular_cycles)
//! - Pruning behält profitable Cycles (enumerate_cycles_generic)
//! - N-Hop-Enumeration findet erwartete Cycles (enumerate_cycles_generic)

use anyhow::Result;
use async_trait::async_trait;
use ironcrab::solana::arbitrage::ArbitrageEngine;
use ironcrab::solana::dex::{Dex, Quote};
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::instruction::Instruction;
use std::sync::Arc;

#[derive(Clone)]
struct MockDex {
    pair: (String, String),
    out_amount: u64,
}

#[async_trait]
impl Dex for MockDex {
    async fn refresh_pools(&self) -> Result<()> {
        Ok(())
    }
    async fn quote_exact_in(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount_in: u64,
    ) -> Result<Option<Quote>> {
        if (input_mint, output_mint) == (self.pair.0.as_str(), self.pair.1.as_str()) {
            Ok(Some(Quote {
                amount_out: self.out_amount + amount_in / 1000,
                price_impact_bps: 10,
                route: vec!["mock".into()],
                fee_bps: 30,
                in_reserve: 1_000_000_000,
                out_reserve: 1_000_000_000,
                input_mint: input_mint.into(),
                output_mint: output_mint.into(),
                tick_spacing: None,
            }))
        } else {
            Ok(None)
        }
    }
    fn build_swap_ix(&self, _i: &str, _o: &str, _a: u64, _m: u64) -> Result<Vec<Instruction>> {
        Ok(vec![])
    }
    fn list_pairs(&self) -> Vec<(String, String)> {
        vec![self.pair.clone()]
    }
}

#[derive(Clone)]
struct EdgeDex {
    a: String,
    b: String,
    mul_bps: u64,
}

#[async_trait]
impl Dex for EdgeDex {
    async fn refresh_pools(&self) -> Result<()> {
        Ok(())
    }
    async fn quote_exact_in(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount_in: u64,
    ) -> Result<Option<Quote>> {
        if input_mint == self.a && output_mint == self.b {
            let out = amount_in.saturating_mul(self.mul_bps) / 10_000;
            return Ok(Some(Quote {
                amount_out: out,
                price_impact_bps: 10,
                route: vec!["e".into()],
                fee_bps: 30,
                in_reserve: 1_000_000_000,
                out_reserve: 1_000_000_000,
                input_mint: input_mint.into(),
                output_mint: output_mint.into(),
                tick_spacing: None,
            }));
        }
        if input_mint == self.b && output_mint == self.a {
            let out = amount_in.saturating_mul(10_000) / self.mul_bps.max(1);
            return Ok(Some(Quote {
                amount_out: out,
                price_impact_bps: 10,
                route: vec!["e".into()],
                fee_bps: 30,
                in_reserve: 1_000_000_000,
                out_reserve: 1_000_000_000,
                input_mint: input_mint.into(),
                output_mint: output_mint.into(),
                tick_spacing: None,
            }));
        }
        Ok(None)
    }
    fn build_swap_ix(&self, _i: &str, _o: &str, _a: u64, _m: u64) -> Result<Vec<Instruction>> {
        Ok(vec![])
    }
    fn list_pairs(&self) -> Vec<(String, String)> {
        vec![(self.a.clone(), self.b.clone())]
    }
}

/// A.14: aggregate_best_edges liefert pro Pair den Quote mit maximalem amount_out.
#[tokio::test]
async fn aggregate_picks_higher_output() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let d1 = Arc::new(MockDex {
        pair: ("A".into(), "B".into()),
        out_amount: 10_000,
    });
    let d2 = Arc::new(MockDex {
        pair: ("A".into(), "B".into()),
        out_amount: 12_000,
    });
    let engine = ArbitrageEngine::new(rpc, vec![d1, d2]);
    let res = engine
        .aggregate_best_edges(&[("A".into(), "B".into())], 100_000)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert!(
        res[0].quote.amount_out >= 12_000 + 100_000 / 1000 - 1,
        "aggregate must pick higher output"
    );
}

/// A.15: rank_triangular_cycles sortiert Cycles absteigend nach Profit.
#[tokio::test]
async fn profit_ranking_orders_cycles() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let connectors: Vec<Arc<dyn Dex>> = vec![
        Arc::new(EdgeDex {
            a: "A".into(),
            b: "B".into(),
            mul_bps: 10_200,
        }),
        Arc::new(EdgeDex {
            a: "B".into(),
            b: "C".into(),
            mul_bps: 10_100,
        }),
        Arc::new(EdgeDex {
            a: "C".into(),
            b: "A".into(),
            mul_bps: 10_200,
        }),
        Arc::new(EdgeDex {
            a: "A".into(),
            b: "X".into(),
            mul_bps: 10_300,
        }),
        Arc::new(EdgeDex {
            a: "X".into(),
            b: "Y".into(),
            mul_bps: 10_200,
        }),
        Arc::new(EdgeDex {
            a: "Y".into(),
            b: "A".into(),
            mul_bps: 10_200,
        }),
    ];
    let engine = ArbitrageEngine::new(rpc, connectors).with_profit_params(0, 0);
    let ranked = engine
        .rank_triangular_cycles(&["A".into()], 1_000_000u64, 5)
        .await
        .unwrap();
    assert!(!ranked.is_empty());
    let top = &ranked[0];
    let (_a, m1, m2) = &top.path;
    assert!(
        *m1 == "X" || *m2 == "Y" || *m1 == "Y" || *m2 == "X",
        "expected higher profit cycle with X/Y in top rank"
    );
    assert!(top.gross_profit > 0);
}

/// A.16: Dominance-Pruning behält mindestens einen profitable Cycle.
#[tokio::test]
async fn pruning_keeps_profitable_cycle() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let connectors: Vec<Arc<dyn Dex>> = vec![
        Arc::new(EdgeDex {
            a: "A".into(),
            b: "B".into(),
            mul_bps: 10_200,
        }),
        Arc::new(EdgeDex {
            a: "B".into(),
            b: "C".into(),
            mul_bps: 10_100,
        }),
        Arc::new(EdgeDex {
            a: "C".into(),
            b: "A".into(),
            mul_bps: 10_150,
        }),
        Arc::new(EdgeDex {
            a: "A".into(),
            b: "B".into(),
            mul_bps: 10_050,
        }),
        Arc::new(EdgeDex {
            a: "B".into(),
            b: "C".into(),
            mul_bps: 10_020,
        }),
    ];
    let engine = ArbitrageEngine::new(rpc, connectors).with_profit_params(0, 0);
    let cycles = engine
        .enumerate_cycles_generic(&["A".into()], 1_000_000, 5, 50)
        .await
        .unwrap();
    assert!(
        cycles
            .iter()
            .any(|c| c.path.len() == 4 && c.gross_profit > 0),
        "expected profitable triangular cycle retained"
    );
}

/// A.17: enumerate_cycles_generic findet 4-Hop-Cycle A->B->C->D->A.
#[tokio::test]
async fn enumerate_4hop_cycle() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let connectors: Vec<Arc<dyn Dex>> = vec![
        Arc::new(EdgeDex {
            a: "A".into(),
            b: "B".into(),
            mul_bps: 10_200,
        }),
        Arc::new(EdgeDex {
            a: "B".into(),
            b: "C".into(),
            mul_bps: 10_100,
        }),
        Arc::new(EdgeDex {
            a: "C".into(),
            b: "D".into(),
            mul_bps: 10_050,
        }),
        Arc::new(EdgeDex {
            a: "D".into(),
            b: "A".into(),
            mul_bps: 10_020,
        }),
    ];
    let engine = ArbitrageEngine::new(rpc, connectors).with_profit_params(0, 0);
    let cycles = engine
        .enumerate_cycles_generic(&["A".into()], 1_000_000u64, 5, 20)
        .await
        .unwrap();
    assert!(!cycles.is_empty());
    let has_abcd = cycles.iter().any(|c| {
        c.path.len() == 5
            && c.path[0] == "A"
            && c.path[1] == "B"
            && c.path[2] == "C"
            && c.path[3] == "D"
            && c.path[4] == "A"
    });
    assert!(has_abcd, "expected A->B->C->D->A cycle present");
    let profitable = cycles.iter().any(|c| c.gross_profit > 0);
    assert!(profitable, "expected positive profit on 4-hop cycle");
}
