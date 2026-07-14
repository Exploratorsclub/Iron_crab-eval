//! Invarianten I-MD-7 / I-MD-8: Hard admission cap + priority/LRU eviction (Blackbox).
//!
//! API-Grenze: `ironcrab::market_data::track::{DesiredExplicitSet, ConsumerId, PinPriority}`.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Assertions an oeffentlicher Track-API, keine privaten Impl-Strukturen.

use ironcrab::market_data::track::{
    pin_priority_from_consumer, ConsumerId, DesiredExplicitSet, PinPriority,
};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn pool(seed: u8) -> Pubkey {
    Pubkey::new_from_array([
        0x50,
        seed,
        seed.wrapping_mul(3),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        seed,
    ])
}

fn assert_never_over_cap(set: &DesiredExplicitSet) {
    assert!(
        set.len() <= set.max_explicit_pubkeys(),
        "dedup explicit set size {} exceeds cap {}",
        set.len(),
        set.max_explicit_pubkeys()
    );
}

/// Spec contract helper: pool account legs are admitted atomically or not at all (I-MD-7).
fn try_admit_pool_group(
    set: &mut DesiredExplicitSet,
    consumer: ConsumerId,
    pool_pk: Pubkey,
    legs: &[Pubkey],
) -> bool {
    let before = set.clone();
    for leg in legs {
        let was_present = set.contains(leg);
        let changed = set.insert(*leg, consumer, Some(pool_pk));
        if !was_present && !changed {
            *set = before;
            return false;
        }
    }
    true
}

/// Deterministic LCG for pseudo-random operation sequences (no external rand crate).
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.0 >> 16) as u32
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next_u32() as usize) % upper
    }
}

#[derive(Clone, Copy)]
enum Op {
    AdmitSingle,
    AdmitGroup,
    RemoveSingle,
    ShrinkCap,
    RestoreCap,
}

/// I-MD-7: beliebige Sequenz admit/remove/shrink/restore → dedup set nie > cap.
#[test]
fn i_md_7_arbitrary_sequences_never_exceed_cap() {
    const BASE_CAP: usize = 6;
    let consumers = [ConsumerId::Wallet, ConsumerId::Momentum, ConsumerId::Arb];
    let mut rng = Lcg::new(0x4D44_7FAD);

    for seed in 0u8..24 {
        let mut set = DesiredExplicitSet::new(BASE_CAP);
        let mut cap = BASE_CAP;
        let mut known: Vec<Pubkey> = (0..16).map(|i| pk(seed.wrapping_add(i))).collect();

        for step in 0..80 {
            let op = match step % 5 {
                0 => Op::AdmitSingle,
                1 => Op::AdmitGroup,
                2 => Op::RemoveSingle,
                3 => Op::ShrinkCap,
                _ => Op::RestoreCap,
            };

            match op {
                Op::AdmitSingle => {
                    let idx = rng.next_usize(known.len());
                    let consumer = consumers[rng.next_usize(consumers.len())];
                    let _ = set.insert(known[idx], consumer, Some(pool(seed)));
                }
                Op::AdmitGroup => {
                    let group_size = 2 + rng.next_usize(3);
                    let start = rng.next_usize(known.len().saturating_sub(group_size));
                    let legs: Vec<Pubkey> = known[start..start + group_size].to_vec();
                    let consumer = consumers[rng.next_usize(consumers.len())];
                    let _ = try_admit_pool_group(
                        &mut set,
                        consumer,
                        pool(seed.wrapping_add(step as u8)),
                        &legs,
                    );
                }
                Op::RemoveSingle => {
                    if known.is_empty() {
                        continue;
                    }
                    let idx = rng.next_usize(known.len());
                    let consumer = consumers[rng.next_usize(consumers.len())];
                    let _ = set.remove(known[idx], consumer);
                }
                Op::ShrinkCap => {
                    let new_cap = 1 + rng.next_usize(cap.max(1));
                    cap = new_cap;
                    set.set_max_explicit_pubkeys(cap);
                }
                Op::RestoreCap => {
                    cap = BASE_CAP + rng.next_usize(4);
                    set.set_max_explicit_pubkeys(cap);
                }
            }

            assert_never_over_cap(&set);

            if step % 11 == 0 {
                let extra = pk(seed.wrapping_add(200).wrapping_add(step as u8));
                known.push(extra);
            }
        }
    }
}

/// I-MD-8: Wallet wird unter Cap-Druck nicht verdraengt.
#[test]
fn i_md_8_wallet_never_evicted_under_cap_pressure() {
    let wallet = pk(1);
    let momentum: Vec<Pubkey> = (2..10).map(pk).collect();
    let arb: Vec<Pubkey> = (10..18).map(pk).collect();

    let mut set = DesiredExplicitSet::new(4);
    assert!(set.insert(wallet, ConsumerId::Wallet, None));

    for pk in &momentum[..3] {
        assert!(set.insert(*pk, ConsumerId::Momentum, Some(pool(1))));
    }
    assert_eq!(set.len(), 4);
    assert!(set.contains(&wallet));

    for pk in momentum.iter().skip(3) {
        let _ = set.insert(*pk, ConsumerId::Momentum, Some(pool(2)));
        assert!(
            set.contains(&wallet),
            "wallet must survive momentum admissions under cap pressure"
        );
    }

    for pk in &arb {
        let _ = set.insert(*pk, ConsumerId::Arb, Some(pool(3)));
        assert!(
            set.contains(&wallet),
            "wallet must survive arb admissions under cap pressure"
        );
    }

    set.set_max_explicit_pubkeys(2);
    assert!(
        set.contains(&wallet),
        "wallet must survive cap shrink when mixed consumers are present"
    );
    assert_never_over_cap(&set);
}

/// I-MD-8: Momentum evictet Arb; Arb evictet nicht Momentum.
#[test]
fn i_md_8_priority_momentum_over_arb_tracker() {
    let cap = 3;
    let mut set = DesiredExplicitSet::new(cap);

    let m1 = pk(10);
    let m2 = pk(11);
    let a1 = pk(20);
    let a2 = pk(21);
    let a3 = pk(22);

    assert!(set.insert(m1, ConsumerId::Momentum, Some(pool(1))));
    assert!(set.insert(a1, ConsumerId::Arb, Some(pool(2))));
    assert!(set.insert(a2, ConsumerId::Arb, Some(pool(3))));
    assert_eq!(set.len(), cap);

    assert!(set.insert(m2, ConsumerId::Momentum, Some(pool(4))));
    assert!(set.contains(&m1));
    assert!(set.contains(&m2));
    assert!(
        !set.contains(&a1) || !set.contains(&a2),
        "momentum insert must evict an arb row"
    );
    assert_never_over_cap(&set);

    let mut set2 = DesiredExplicitSet::new(cap);
    assert!(set2.insert(m1, ConsumerId::Momentum, Some(pool(5))));
    assert!(set2.insert(m2, ConsumerId::Momentum, Some(pool(6))));
    assert!(set2.insert(a1, ConsumerId::Arb, Some(pool(7))));
    assert_eq!(set2.len(), cap);

    assert!(set2.insert(a2, ConsumerId::Arb, Some(pool(8))));
    assert!(set2.contains(&m1));
    assert!(set2.contains(&m2));
    assert!(
        !set2.contains(&a1),
        "arb insert must evict arb/tracker, not momentum"
    );
    assert!(set2.contains(&a2));

    assert!(set2.insert(a3, ConsumerId::Arb, Some(pool(9))));
    assert!(set2.contains(&m1));
    assert!(set2.contains(&m2));
    assert_never_over_cap(&set2);
}

/// I-MD-8: geteilte Pubkeys besitzen Owner-Referenzen (Consumer-Refcount).
#[test]
fn i_md_8_shared_pubkey_owner_references() {
    let shared = pk(42);
    let mut set = DesiredExplicitSet::new(8);

    assert!(set.insert(shared, ConsumerId::Wallet, None));
    assert!(!set.insert(shared, ConsumerId::Momentum, Some(pool(1))));
    assert_eq!(set.len(), 1);

    let owners = set.consumers_of(&shared).expect("shared pubkey must exist");
    assert!(owners.contains(&ConsumerId::Wallet));
    assert!(owners.contains(&ConsumerId::Momentum));
    assert_eq!(
        pin_priority_from_consumer(ConsumerId::Wallet),
        PinPriority::Wallet
    );

    assert!(!set.remove(shared, ConsumerId::Momentum));
    assert!(set.contains(&shared));
    let owners = set.consumers_of(&shared).expect("wallet owner remains");
    assert!(owners.contains(&ConsumerId::Wallet));
    assert!(!owners.contains(&ConsumerId::Momentum));

    assert!(set.remove(shared, ConsumerId::Wallet));
    assert!(!set.contains(&shared));
}

/// I-MD-7: abgelehnte Pool-Gruppe mutiert keinen publizierten Snapshot.
#[test]
fn i_md_7_rejected_pool_group_leaves_snapshot_unchanged() {
    let cap = 3;
    let mut set = DesiredExplicitSet::new(cap);

    let pool_a = pool(1);
    let legs_a = [pk(1), pk(2), pk(3)];
    assert!(try_admit_pool_group(
        &mut set,
        ConsumerId::Momentum,
        pool_a,
        &legs_a
    ));
    assert_eq!(set.len(), cap);

    let snapshot_before = set.snapshot_pubkeys();
    let pool_b = pool(2);
    let legs_b = [pk(4), pk(5), pk(6), pk(7)];
    assert!(!try_admit_pool_group(
        &mut set,
        ConsumerId::Arb,
        pool_b,
        &legs_b
    ));
    assert_eq!(set.snapshot_pubkeys(), snapshot_before);
    assert_eq!(set.len(), cap);
    assert_never_over_cap(&set);
}

/// I-MD-8: Wallet-only over-cap → fail-closed (kein stilles Ueberlaufen).
#[test]
fn i_md_8_wallet_only_over_cap_fail_closed() {
    let cap = 2;
    let mut set = DesiredExplicitSet::new(cap);
    let w1 = pk(1);
    let w2 = pk(2);
    let w3 = pk(3);

    assert!(set.insert(w1, ConsumerId::Wallet, None));
    assert!(set.insert(w2, ConsumerId::Wallet, None));
    assert_eq!(set.len(), cap);

    assert!(
        !set.insert(w3, ConsumerId::Wallet, None),
        "third wallet must be rejected when cap is full of wallet pins"
    );
    assert_eq!(set.len(), cap);
    assert!(!set.contains(&w3));
    assert_never_over_cap(&set);
}

/// I-MD-7/I-MD-8: Restore nach Oversubscription konvergiert oder bleibt fail-closed.
#[test]
fn i_md_7_restore_after_oversubscribed_converges_or_fail_closed() {
    let mut set = DesiredExplicitSet::new(5);
    let wallet = pk(1);
    let momentum: Vec<Pubkey> = (2..8).map(pk).collect();

    assert!(set.insert(wallet, ConsumerId::Wallet, None));
    for pk in &momentum {
        let _ = set.insert(*pk, ConsumerId::Momentum, Some(pool(1)));
    }
    assert_never_over_cap(&set);
    let wallet_present = set.contains(&wallet);

    set.set_max_explicit_pubkeys(2);
    assert_never_over_cap(&set);
    assert!(set.len() <= 2);
    if wallet_present {
        assert!(
            set.contains(&wallet),
            "wallet must not be evicted during shrink when it was admitted"
        );
    }

    set.set_max_explicit_pubkeys(5);
    assert_never_over_cap(&set);

    let fresh = pk(99);
    let admitted = set.insert(fresh, ConsumerId::Momentum, Some(pool(9)));
    if set.len() < set.max_explicit_pubkeys() {
        assert!(
            admitted,
            "restore cap must admit when capacity is available"
        );
    }
    assert_never_over_cap(&set);

    let mut wallet_only = DesiredExplicitSet::new(2);
    assert!(wallet_only.insert(pk(50), ConsumerId::Wallet, None));
    assert!(wallet_only.insert(pk(51), ConsumerId::Wallet, None));
    wallet_only.set_max_explicit_pubkeys(1);
    assert_never_over_cap(&wallet_only);
    assert!(
        !wallet_only.insert(pk(52), ConsumerId::Wallet, None),
        "wallet-only oversubscribed restore path stays fail-closed on new wallet admit"
    );
}

/// Regression: PinPriority ordering matches documented protection chain.
#[test]
fn i_md_8_pin_priority_ordering_contract() {
    assert!(PinPriority::Wallet < PinPriority::Momentum);
    assert!(PinPriority::Momentum < PinPriority::Arb);
    assert!(PinPriority::Arb < PinPriority::Tracker);
    assert_eq!(
        pin_priority_from_consumer(ConsumerId::Wallet),
        PinPriority::Wallet
    );
    assert_eq!(
        pin_priority_from_consumer(ConsumerId::Momentum),
        PinPriority::Momentum
    );
    assert_eq!(
        pin_priority_from_consumer(ConsumerId::Arb),
        PinPriority::Arb
    );
}

/// I-MD-7: dedupliziertes Set — insert gleicher Pubkey mit zweitem Consumer erhoeht len nicht.
#[test]
fn i_md_7_dedup_set_single_physical_pubkey_per_key() {
    let mut set = DesiredExplicitSet::new(4);
    let shared = pk(7);
    let other = pk(8);

    assert!(set.insert(shared, ConsumerId::Wallet, None));
    assert!(!set.insert(shared, ConsumerId::Arb, Some(pool(1))));
    assert!(set.insert(other, ConsumerId::Momentum, Some(pool(2))));

    let keys: HashSet<Pubkey> = set.snapshot_pubkeys();
    assert_eq!(keys.len(), set.len());
    assert_eq!(set.len(), 2);

    let mut consumer_map: HashMap<Pubkey, usize> = HashMap::new();
    for pk in keys {
        let count = set.consumers_of(&pk).map(|c| c.len()).unwrap_or(0);
        consumer_map.insert(pk, count);
    }
    assert_eq!(consumer_map[&shared], 2);
    assert_eq!(consumer_map[&other], 1);
}
