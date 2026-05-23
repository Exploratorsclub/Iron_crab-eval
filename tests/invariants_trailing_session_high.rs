//! Trailing Session High + quote-first Exits (Spec-Policy, Eval § I-13/I-14)
//!
//! Modelliert die **Policy** aus dem Trailing-Session-High-Handoff (PR #148):
//! - `session_high_tps` nur aus Entry-Fill + Positions-Pool-Trades (niedrigster tps = bester Holder-Preis).
//! - PoolCache-Reserve-Marks setzen nur `current_mark_tps`, **nicht** Session-High.
//! - Trailing-Aktivierung: `pnl_pct(entry, session_high_tps) >= trailing_activation_pct`.
//! - Trailing-Trigger: `drawdown_from_ath_pct(session_high_tps, executable_quote_tps) >= trailing_stop_pct`.
//! - Stop-Loss bleibt quote-first vs. Entry: `pnl_pct(entry, quote_tps)` (keine Abschwächung über Mark).
//!
//! Blackbox: nur `ironcrab::execution::tokens_per_sol::*` (öffentliche Math-API), kein Bin-/Position-Interna.

use ironcrab::execution::tokens_per_sol::{drawdown_from_ath_pct, pnl_pct, updated_highest_price};

/// Spec-State für eine einzelne Position (tps-Raum, I-14).
#[derive(Clone, Debug)]
struct TrailingSessionHighFixture {
    entry_tps: f64,
    /// Bester Holder-Preis seit Entry auf dem Positions-Pool: **niedrigster** tps (I-14).
    session_high_tps: f64,
    /// Letzter Mark aus Trade oder PoolCache (kann von Session-High abweichen).
    current_mark_tps: f64,
}

impl TrailingSessionHighFixture {
    fn new() -> Self {
        Self {
            entry_tps: 0.0,
            session_high_tps: 0.0,
            current_mark_tps: 0.0,
        }
    }

    /// Entry-Fill auf Positions-Pool: Entry und Session-High starten am Fill-tps.
    fn apply_entry_fill(&mut self, tps: f64) {
        self.entry_tps = tps;
        self.session_high_tps = tps;
        self.current_mark_tps = tps;
    }

    /// Trade-Event auf Positions-Pool (slot > entry): Mark **und** Session-High aktualisieren.
    fn apply_trade_on_position_pool(&mut self, tps: f64) {
        self.current_mark_tps = tps;
        self.session_high_tps = updated_highest_price(self.session_high_tps, tps);
    }

    /// PoolCache-Reserve-Mark: nur Mark — Session-High bleibt (I-13: kein Trailing-ATH aus Reserves).
    fn apply_pool_cache_mark(&mut self, tps: f64) {
        self.current_mark_tps = tps;
    }

    /// Executable Quote: nur für Trigger-/Stop-Evaluation; **kein** Session-High-Update (Spec).
    fn apply_executable_quote(&self, _quote_tps: f64) {
        // absichtlich kein State-Mutate: Quote-first Evaluation liest Argument out-of-band
    }

    fn trailing_activation_met(&self, threshold_pct: f64) -> bool {
        pnl_pct(self.entry_tps, self.session_high_tps) >= threshold_pct
    }

    fn trailing_trigger_met(&self, stop_pct: f64, quote_tps: f64) -> bool {
        drawdown_from_ath_pct(self.session_high_tps, quote_tps) >= stop_pct
    }
}

/// Reserve-Mark verbessert tps (Mark sinkt), Session-High bleibt beim Entry-Wert → keine Aktivierung bei realistischem Schwellenwert.
#[test]
fn reserve_mark_does_not_advance_session_high() {
    // Policy: PoolCache darf current_mark_tps setzen, nicht session_high_tps.
    let mut f = TrailingSessionHighFixture::new();
    f.apply_entry_fill(100.0);
    // „Besserer“ Mark: 90 tps < 100 tps (wertvoller), aber ohne Trade kein neues Session-High.
    f.apply_pool_cache_mark(90.0);
    assert!(
        (f.session_high_tps - 100.0).abs() < 1e-9,
        "Session-High darf sich nicht durch Reserve-Mark verbessern"
    );
    assert!((f.current_mark_tps - 90.0).abs() < 1e-9);

    let activation_threshold_pct = 1.0;
    assert!(
        !f.trailing_activation_met(activation_threshold_pct),
        "PnL vs Session-High ist 0 %; ohne Trade-Peak keine Aktivierung (Schwelle {} %)",
        activation_threshold_pct
    );
    assert!((pnl_pct(f.entry_tps, f.session_high_tps) - 0.0).abs() < 1e-9);
}

/// Trade auf Positions-Pool senkt tps → Session-High folgt → Aktivierung sobald Schwelle erreicht.
#[test]
fn trade_advances_session_high_and_activates_trailing() {
    let mut f = TrailingSessionHighFixture::new();
    f.apply_entry_fill(100.0);
    f.apply_trade_on_position_pool(95.0);
    assert!((f.session_high_tps - 95.0).abs() < 1e-9);

    let pnl_vs_session_high = pnl_pct(f.entry_tps, f.session_high_tps);
    assert!(
        (pnl_vs_session_high - (100.0 / 95.0 - 1.0) * 100.0).abs() < 1e-6,
        "Aktivierung nutzt entry vs session_high_tps"
    );

    let activation_threshold_pct = 5.0;
    assert!(
        f.trailing_activation_met(activation_threshold_pct),
        "Nach Trade-Peak soll Trailing-Aktivierung bei {} % greifen (PnL ≈ {:.2} %)",
        activation_threshold_pct,
        pnl_vs_session_high
    );
}

/// Session-High stammt aus Trade; Quote erzeugt Drawdown vs dieses High — Quote setzt kein Session-High.
#[test]
fn quote_trigger_uses_session_high_not_quote_as_high() {
    let mut f = TrailingSessionHighFixture::new();
    f.apply_entry_fill(100.0);
    f.apply_trade_on_position_pool(90.0);
    assert!((f.session_high_tps - 90.0).abs() < 1e-9);

    // Schlechter ausführbarer Quote (höheres tps): Drawdown vs Session-High 90, nicht „Quote als ATH“.
    let quote_tps = 108.0;
    f.apply_executable_quote(quote_tps);

    let dd = drawdown_from_ath_pct(90.0, quote_tps);
    assert!(
        (dd - 20.0).abs() < 1e-9,
        "Erwarteter Drawdown von ATH 90 zu Quote 108"
    );

    let trailing_stop_pct = 15.0;
    assert!(
        f.trailing_trigger_met(trailing_stop_pct, quote_tps),
        "Trigger: drawdown_from_ath_pct(session_high, quote) >= stop (quote-first, guarded)"
    );

    // Session-High unverändert trotz schlechterem Quote.
    assert!((f.session_high_tps - 90.0).abs() < 1e-9);
}

/// Modestes Trade-Peak (+0,5 % PnL vs Entry), dann starke Reserve-Spike nur auf dem Mark — Trailing nicht aktiviert.
#[test]
fn cat_class_modest_trade_then_reserve_spike_no_activation() {
    let mut f = TrailingSessionHighFixture::new();
    f.apply_entry_fill(100.0);

    // +0,5 % PnL: (100/tps - 1)*100 = 0.5 → tps = 100/1.005
    let tps_after_modest_trade = 100.0 / 1.005;
    f.apply_trade_on_position_pool(tps_after_modest_trade);
    assert!(f.session_high_tps < 100.0);

    // +10 % PnL nur auf Mark (Reserve): (100/mark - 1)*100 = 10 → mark = 100/1.10
    let mark_after_spike = 100.0 / 1.10;
    f.apply_pool_cache_mark(mark_after_spike);

    assert!(
        (f.session_high_tps - tps_after_modest_trade).abs() < 1e-6,
        "Reserve-Spike darf Session-High nicht auf {:.6} ziehen",
        mark_after_spike
    );

    let activation_threshold_pct = 1.0;
    assert!(
        !f.trailing_activation_met(activation_threshold_pct),
        "Session-High-Peak bleibt bei ~0,5 %; Aktivierung bei {} % muss false sein",
        activation_threshold_pct
    );
    let pnl_at_session_high = pnl_pct(f.entry_tps, f.session_high_tps);
    assert!(
        pnl_at_session_high < activation_threshold_pct - 1e-6,
        "PnL vs Session-High ≈ {:.4} % < {} %",
        pnl_at_session_high,
        activation_threshold_pct
    );
}

/// Stop-Loss: schlechter **executable** Quote vs Entry — Entscheid über `pnl_pct(entry, quote_tps)`, nicht über milden Mark.
#[test]
fn stop_loss_quote_first_regression() {
    let mut f = TrailingSessionHighFixture::new();
    f.apply_entry_fill(100.0);
    f.apply_pool_cache_mark(99.0);

    let quote_tps = 150.0;
    let pnl_quote = pnl_pct(f.entry_tps, quote_tps);
    assert!(
        (pnl_quote - (-33.33333333333333)).abs() < 1e-6,
        "Quote-first PnL vs Entry"
    );

    let stop_loss_threshold_pct = 25.0;
    assert!(
        pnl_quote <= -stop_loss_threshold_pct,
        "STOP: Verlust über Quote ({:.2} %) >= Schwelle {} %; milder Mark {:.1} täuscht nicht",
        pnl_quote,
        stop_loss_threshold_pct,
        f.current_mark_tps
    );

    let pnl_mark_only = pnl_pct(f.entry_tps, f.current_mark_tps);
    assert!(
        pnl_mark_only > pnl_quote,
        "Mark allein wäre weniger negativ — Stop muss Quote-first folgen"
    );
}
