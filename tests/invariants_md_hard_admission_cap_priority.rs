//! Invarianten I-MD-7 / I-MD-8: Hard admission cap + priority/LRU eviction (Blackbox).
//!
//! API-Grenze: `ironcrab::market_data::track::{FixedCapAdmission, try_admit_owner_group, ...}`.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Assertions an oeffentlicher Track-API, keine privaten Impl-Strukturen.

use ironcrab::market_data::track::{
    admitted_pubkey_set, apply_cap_shrink, pin_priority_from_consumer,
    restore_admission_from_owner_groups, try_admit_owner_group, AdmissionRestoreResult,
    CapShrinkResult, ConsumerId, EvictingAdmissionResult, ExplicitConsumer, ExplicitOwner,
    ExplicitOwnerKey, FixedCapAdmission, OwnerGroupSnapshot, PinPriority,
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

fn wallet_owner() -> ExplicitOwner {
    ExplicitOwner {
        consumer: ExplicitConsumer::Wallet,
        owner_key: ExplicitOwnerKey::Wallet,
    }
}

fn pool_owner(consumer: ExplicitConsumer, pool_pk: Pubkey) -> ExplicitOwner {
    ExplicitOwner {
        consumer,
        owner_key: ExplicitOwnerKey::Pool(pool_pk),
    }
}

fn assert_never_over_cap(admission: &FixedCapAdmission) {
    assert!(
        admission.len() <= admission.cap(),
        "admitted physical set size {} exceeds cap {}",
        admission.len(),
        admission.cap()
    );
}

fn admit_pool_group(
    admission: &mut FixedCapAdmission,
    consumer: ExplicitConsumer,
    pool_pk: Pubkey,
    legs: &[Pubkey],
) -> bool {
    try_admit_owner_group(admission, pool_owner(consumer, pool_pk), legs.to_vec())
}

fn admit_wallet_mints(admission: &mut FixedCapAdmission, mints: &[Pubkey]) -> bool {
    try_admit_owner_group(admission, wallet_owner(), mints.to_vec())
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
    RemoveOwner,
    ShrinkCap,
    RestoreCap,
}

const EXPLICIT_CONSUMERS: [ExplicitConsumer; 4] = [
    ExplicitConsumer::Wallet,
    ExplicitConsumer::Momentum,
    ExplicitConsumer::Arb,
    ExplicitConsumer::Tracker,
];

/// I-MD-7: beliebige Sequenz admit/remove/shrink/restore → dedup set nie > cap.
#[test]
fn i_md_7_arbitrary_sequences_never_exceed_cap() {
    const BASE_CAP: usize = 6;
    let mut rng = Lcg::new(0x4D44_7FAD);

    for seed in 0u8..24 {
        let mut admission = FixedCapAdmission::new(BASE_CAP);
        let mut known_owners: Vec<ExplicitOwner> = Vec::new();
        let mut known: Vec<Pubkey> = (0..16).map(|i| pk(seed.wrapping_add(i))).collect();

        for step in 0..80 {
            let op = match step % 5 {
                0 => Op::AdmitSingle,
                1 => Op::AdmitGroup,
                2 => Op::RemoveOwner,
                3 => Op::ShrinkCap,
                _ => Op::RestoreCap,
            };

            match op {
                Op::AdmitSingle => {
                    let idx = rng.next_usize(known.len());
                    let consumer = EXPLICIT_CONSUMERS[rng.next_usize(EXPLICIT_CONSUMERS.len())];
                    let pool_pk = pool(seed.wrapping_add(step as u8));
                    let owner = match consumer {
                        ExplicitConsumer::Wallet => wallet_owner(),
                        _ => pool_owner(consumer, pool_pk),
                    };
                    let _ = try_admit_owner_group(&mut admission, owner.clone(), vec![known[idx]]);
                    if !known_owners.contains(&owner) {
                        known_owners.push(owner);
                    }
                }
                Op::AdmitGroup => {
                    let group_size = 2 + rng.next_usize(3);
                    let start = rng.next_usize(known.len().saturating_sub(group_size));
                    let legs: Vec<Pubkey> = known[start..start + group_size].to_vec();
                    let consumer = EXPLICIT_CONSUMERS[rng.next_usize(3)];
                    let pool_pk = pool(seed.wrapping_add(step as u8));
                    let owner = pool_owner(consumer, pool_pk);
                    let _ = try_admit_owner_group(&mut admission, owner.clone(), legs);
                    if !known_owners.contains(&owner) {
                        known_owners.push(owner);
                    }
                }
                Op::RemoveOwner => {
                    if known_owners.is_empty() {
                        continue;
                    }
                    let idx = rng.next_usize(known_owners.len());
                    let owner = known_owners[idx].clone();
                    let _ = admission.remove_group(owner);
                }
                Op::ShrinkCap => {
                    let new_cap = 1 + rng.next_usize(admission.cap().max(1));
                    let _ = apply_cap_shrink(&mut admission, new_cap);
                }
                Op::RestoreCap => {
                    let target = BASE_CAP + rng.next_usize(4);
                    if target > admission.cap() {
                        let groups = admission.snapshot_owner_groups();
                        let mut fresh = FixedCapAdmission::new(target);
                        let _ = restore_admission_from_owner_groups(&mut fresh, &groups);
                        admission = fresh;
                    } else {
                        let _ = apply_cap_shrink(&mut admission, target);
                    }
                }
            }

            assert_never_over_cap(&admission);

            if step % 11 == 0 {
                known.push(pk(seed.wrapping_add(200).wrapping_add(step as u8)));
            }
        }
    }
}

/// I-MD-8: Wallet wird unter Cap-Druck nicht verdraengt.
#[test]
fn i_md_8_wallet_never_evicted_under_cap_pressure() {
    let wallet_mint = pk(1);
    let momentum: Vec<Pubkey> = (2..10).map(pk).collect();
    let arb: Vec<Pubkey> = (10..18).map(pk).collect();

    let mut admission = FixedCapAdmission::new(4);
    assert!(admit_wallet_mints(&mut admission, &[wallet_mint]));

    for (idx, pk) in momentum.iter().take(3).enumerate() {
        assert!(admit_pool_group(
            &mut admission,
            ExplicitConsumer::Momentum,
            pool(idx as u8 + 1),
            &[*pk],
        ));
    }
    assert_eq!(admission.len(), 4);
    assert!(admission.contains(&wallet_mint));

    for (idx, pk) in momentum.iter().skip(3).enumerate() {
        let _ = admit_pool_group(
            &mut admission,
            ExplicitConsumer::Momentum,
            pool(idx as u8 + 10),
            &[*pk],
        );
        assert!(
            admission.contains(&wallet_mint),
            "wallet must survive momentum admissions under cap pressure"
        );
    }

    for (idx, pk) in arb.iter().enumerate() {
        let _ = admit_pool_group(
            &mut admission,
            ExplicitConsumer::Arb,
            pool(idx as u8 + 20),
            &[*pk],
        );
        assert!(
            admission.contains(&wallet_mint),
            "wallet must survive arb admissions under cap pressure"
        );
    }

    match apply_cap_shrink(&mut admission, 2) {
        CapShrinkResult::Converged { .. } | CapShrinkResult::NoOpAlreadyWithinCap { .. } => {}
        other => panic!("unexpected shrink result: {other:?}"),
    }
    assert!(
        admission.contains(&wallet_mint),
        "wallet must survive cap shrink when mixed consumers are present"
    );
    assert_never_over_cap(&admission);
}

/// I-MD-8: Momentum evictet Arb/Tracker; Arb evictet Tracker nicht Momentum.
#[test]
fn i_md_8_priority_momentum_over_arb_tracker() {
    let cap = 3;
    let mut admission = FixedCapAdmission::new(cap);

    let m1 = pk(10);
    let m2 = pk(11);
    let a1 = pk(20);
    let a2 = pk(21);
    let a3 = pk(22);
    let t1 = pk(30);

    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Momentum,
        pool(1),
        &[m1],
    ));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Arb,
        pool(2),
        &[a1],
    ));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Arb,
        pool(3),
        &[a2],
    ));
    assert_eq!(admission.len(), cap);

    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Momentum,
        pool(4),
        &[m2],
    ));
    assert!(admission.contains(&m1));
    assert!(admission.contains(&m2));
    assert!(
        !admission.contains(&a1) || !admission.contains(&a2),
        "momentum insert must evict an arb row"
    );
    assert_never_over_cap(&admission);

    let mut admission2 = FixedCapAdmission::new(cap);
    assert!(admit_pool_group(
        &mut admission2,
        ExplicitConsumer::Momentum,
        pool(5),
        &[m1],
    ));
    assert!(admit_pool_group(
        &mut admission2,
        ExplicitConsumer::Momentum,
        pool(6),
        &[m2],
    ));
    assert!(admit_pool_group(
        &mut admission2,
        ExplicitConsumer::Arb,
        pool(7),
        &[a1],
    ));
    assert_eq!(admission2.len(), cap);

    assert!(admit_pool_group(
        &mut admission2,
        ExplicitConsumer::Arb,
        pool(8),
        &[a2],
    ));
    assert!(admission2.contains(&m1));
    assert!(admission2.contains(&m2));
    assert!(
        !admission2.contains(&a1),
        "arb insert must evict arb/tracker, not momentum"
    );
    assert!(admission2.contains(&a2));

    assert!(admit_pool_group(
        &mut admission2,
        ExplicitConsumer::Arb,
        pool(9),
        &[a3],
    ));
    assert!(admission2.contains(&m1));
    assert!(admission2.contains(&m2));
    assert_never_over_cap(&admission2);

    let mut admission3 = FixedCapAdmission::new(cap);
    assert!(admit_pool_group(
        &mut admission3,
        ExplicitConsumer::Momentum,
        pool(10),
        &[m1],
    ));
    assert!(admit_pool_group(
        &mut admission3,
        ExplicitConsumer::Tracker,
        pool(11),
        &[t1],
    ));
    assert!(admit_pool_group(
        &mut admission3,
        ExplicitConsumer::Arb,
        pool(12),
        &[a1],
    ));
    assert!(admit_pool_group(
        &mut admission3,
        ExplicitConsumer::Momentum,
        pool(13),
        &[m2],
    ));
    assert!(admission3.contains(&m1));
    assert!(admission3.contains(&m2));
    assert!(
        !admission3.contains(&t1) || !admission3.contains(&a1),
        "momentum must evict tracker/arb, not momentum"
    );
}

/// I-MD-8: geteilte Pubkeys besitzen Owner-Referenzen (Consumer-Refcount).
#[test]
fn i_md_8_shared_pubkey_owner_references() {
    let shared = pk(42);
    let companion = pk(43);
    let mut admission = FixedCapAdmission::new(8);

    assert!(admit_wallet_mints(&mut admission, &[shared]));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Momentum,
        pool(1),
        &[shared, companion],
    ));
    assert_eq!(admission.len(), 2);
    assert_eq!(admission.owner_refcount(&shared), 2);
    assert_eq!(admission.owner_refcount(&companion), 1);

    admission.remove_group(wallet_owner());
    assert!(admission.contains(&shared));
    assert_eq!(admission.owner_refcount(&shared), 1);

    admission.remove_group(pool_owner(ExplicitConsumer::Momentum, pool(1)));
    assert!(!admission.contains(&shared));
    assert!(!admission.contains(&companion));
}

/// I-MD-7: abgelehnte Owner-Gruppe mutiert keinen publizierten Snapshot.
#[test]
fn i_md_7_rejected_pool_group_leaves_snapshot_unchanged() {
    let cap = 3;
    let mut admission = FixedCapAdmission::new(cap);

    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Momentum,
        pool(1),
        &[pk(1), pk(2), pk(3)],
    ));
    assert_eq!(admission.len(), cap);

    let snapshot_before = admitted_pubkey_set(&admission);
    assert!(!admit_wallet_mints(
        &mut admission,
        &[pk(4), pk(5), pk(6), pk(7)],
    ));
    assert_eq!(admitted_pubkey_set(&admission), snapshot_before);
    assert_eq!(admission.len(), cap);
    assert_never_over_cap(&admission);
}

/// I-MD-8: Wallet-only over-cap → fail-closed (kein stilles Ueberlaufen).
#[test]
fn i_md_8_wallet_only_over_cap_fail_closed() {
    let cap = 2;
    let w1 = pk(1);
    let w2 = pk(2);
    let w3 = pk(3);

    let groups = vec![OwnerGroupSnapshot {
        consumer: ExplicitConsumer::Wallet,
        owner_key: ExplicitOwnerKey::Wallet,
        pubkeys: vec![w1, w2, w3],
    }];
    let mut admission = FixedCapAdmission::new(cap);
    assert_eq!(
        restore_admission_from_owner_groups(&mut admission, &groups),
        AdmissionRestoreResult::ProtectedOverflow
    );
    assert_eq!(admission.len(), 0);

    assert!(
        !admit_wallet_mints(&mut admission, &[w1, w2, w3]),
        "fresh wallet group larger than cap must be rejected"
    );
    assert_eq!(admission.len(), 0);

    assert!(admit_wallet_mints(&mut admission, &[w1, w2]));
    assert_eq!(admission.len(), cap);
    assert!(matches!(
        apply_cap_shrink(&mut admission, 1),
        CapShrinkResult::ProtectedOverflow { .. }
    ));
    assert_eq!(admission.len(), cap);
    assert_never_over_cap(&admission);

    match admission.try_admit_with_eviction(
        pool_owner(ExplicitConsumer::Momentum, pool(1)),
        vec![pk(10), pk(11), pk(12)],
    ) {
        EvictingAdmissionResult::RejectedProtected { .. }
        | EvictingAdmissionResult::InsertedWithEviction { .. } => {}
        other => panic!("unexpected evicting result under wallet cap pressure: {other:?}"),
    }
    assert!(
        admission.contains(&w1) && admission.contains(&w2),
        "wallet pubkeys must remain after protected reject paths"
    );
}

/// I-MD-7/I-MD-8: Restore nach Oversubscription konvergiert oder bleibt fail-closed.
#[test]
fn i_md_7_restore_after_oversubscribed_converges_or_fail_closed() {
    let mut admission = FixedCapAdmission::new(5);
    let wallet_mint = pk(1);

    assert!(admit_wallet_mints(&mut admission, &[wallet_mint]));
    for (idx, seed) in (2..8u8).enumerate() {
        let _ = admit_pool_group(
            &mut admission,
            ExplicitConsumer::Momentum,
            pool(idx as u8 + 1),
            &[pk(seed)],
        );
    }
    assert_never_over_cap(&admission);
    assert!(admission.contains(&wallet_mint));

    match apply_cap_shrink(&mut admission, 2) {
        CapShrinkResult::Converged { .. } | CapShrinkResult::NoOpAlreadyWithinCap { .. } => {}
        CapShrinkResult::ProtectedOverflow { .. } => {}
        other => panic!("unexpected shrink: {other:?}"),
    }
    assert_never_over_cap(&admission);
    assert!(admission.len() <= 2);
    assert!(
        admission.contains(&wallet_mint),
        "wallet must not be evicted during shrink when it was admitted"
    );

    let saved = admission.snapshot_owner_groups();
    let mut restored = FixedCapAdmission::new(5);
    assert_eq!(
        restore_admission_from_owner_groups(&mut restored, &saved),
        AdmissionRestoreResult::Restored
    );
    assert_never_over_cap(&restored);

    let fresh = pk(99);
    if restored.len() < restored.cap() {
        assert!(admit_pool_group(
            &mut restored,
            ExplicitConsumer::Momentum,
            pool(99),
            &[fresh],
        ));
    }
    assert_never_over_cap(&restored);

    let mut wallet_only = FixedCapAdmission::new(2);
    assert!(admit_wallet_mints(&mut wallet_only, &[pk(50), pk(51)]));
    assert!(matches!(
        apply_cap_shrink(&mut wallet_only, 1),
        CapShrinkResult::ProtectedOverflow { .. }
    ));
    assert!(
        !admit_wallet_mints(&mut wallet_only, &[pk(50), pk(51), pk(52)]),
        "wallet-only oversubscribed path stays fail-closed on new wallet admit"
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
    assert_eq!(
        pin_priority_from_consumer(ConsumerId::Tracker),
        PinPriority::Tracker
    );
}

/// I-MD-8: Eviction-Planner waehlt nie Wallet als Opfer wenn niedrigere Tiers verfuegbar.
#[test]
fn i_md_8_eviction_planner_wallet_never_victim_when_alternatives_exist() {
    let mut admission = FixedCapAdmission::new(3);
    let wallet_mint = pk(1);
    let arb1 = pk(2);
    let arb2 = pk(3);
    let mom = pk(4);

    assert!(admit_wallet_mints(&mut admission, &[wallet_mint]));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Arb,
        pool(1),
        &[arb1],
    ));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Arb,
        pool(2),
        &[arb2],
    ));
    assert_eq!(admission.len(), 3);

    let incoming = pool_owner(ExplicitConsumer::Momentum, pool(3));
    let plan = admission.plan_admit_with_eviction(incoming.clone(), vec![mom]);
    match plan {
        ironcrab::market_data::track::VictimSelectionResult::Planned(p) => {
            assert!(
                p.victims
                    .iter()
                    .all(|v| v.consumer != ExplicitConsumer::Wallet),
                "planner must not select wallet victims when arb tiers can satisfy demand"
            );
        }
        other => panic!("expected planned eviction, got {other:?}"),
    }

    match admission.try_admit_with_eviction(incoming, vec![mom]) {
        EvictingAdmissionResult::InsertedWithEviction { victims, .. } => {
            assert!(victims
                .iter()
                .all(|v| v.consumer != ExplicitConsumer::Wallet));
            assert!(admission.contains(&wallet_mint));
            assert!(admission.contains(&mom));
        }
        other => panic!("expected evicting insert, got {other:?}"),
    }
}

/// I-MD-7: dedupliziertes Set — gleicher Pubkey unter zwei Ownern zaehlt physisch einmal.
#[test]
fn i_md_7_dedup_set_single_physical_pubkey_per_key() {
    let mut admission = FixedCapAdmission::new(4);
    let shared = pk(7);
    let other = pk(8);

    assert!(admit_wallet_mints(&mut admission, &[shared]));
    assert!(admit_pool_group(
        &mut admission,
        ExplicitConsumer::Arb,
        pool(1),
        &[shared, other],
    ));

    let keys: HashSet<Pubkey> = admitted_pubkey_set(&admission);
    assert_eq!(keys.len(), admission.len());
    assert_eq!(admission.len(), 2);

    let mut consumer_map: HashMap<Pubkey, usize> = HashMap::new();
    for pk in keys {
        consumer_map.insert(pk, admission.owner_refcount(&pk));
    }
    assert_eq!(consumer_map[&shared], 2);
    assert_eq!(consumer_map[&other], 1);
}
