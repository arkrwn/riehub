//! The conformance suite (storage spec, section 1): one function per
//! rule of section 3. Every backend must pass all of it. Enabled by
//! the `conformance` feature for backend crates, and compiled for
//! this crate's own tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::audit::{AuditDraft, AuditHead, AuditRow, ChainMac};
use crate::record::{Expect, Filter, Kind, Page, Record};
use crate::usage::{UsageBucket, UsageGroup, UsageQuery, UsageRow};
use crate::{CounterKey, Store};
use riehub_core::clock::Clock;
use riehub_core::error::Result;
use riehub_core::Owner;
use std::future::Future;
use std::pin::Pin;

/// A store factory. Each call must produce a fresh, empty store.
pub type MakeStore<'a, S> =
    &'a (dyn Fn() -> Pin<Box<dyn Future<Output = Result<S>> + Send>> + Send + Sync);

/// A deterministic, non-cryptographic mac for conformance runs. The
/// real mac implementation comes from riehub-crypto in production.
#[derive(Debug, Clone, Copy)]
pub struct ConformanceMac;

impl ChainMac for ConformanceMac {
    fn compute(&self, prev_mac: &[u8], event: &[u8]) -> [u8; 48] {
        let mut out = [0u8; 48];
        for (i, b) in prev_mac.iter().chain(event.iter()).enumerate() {
            out[i % 48] = out[i % 48].wrapping_add(*b).rotate_left(1);
        }
        out
    }
}

macro_rules! require {
    ($cond:expr, $msg:literal) => {
        if !$cond {
            return Err($msg);
        }
    };
}

/// Runs every rule. Returns the list of failures, empty on success.
pub async fn run<S: Store>(make: MakeStore<'_, S>) -> Vec<&'static str> {
    let mut failures = Vec::new();
    macro_rules! check {
        ($f:expr) => {
            if let Err(e) = $f(make).await {
                failures.push(e);
            }
        };
    }
    check!(case_put_new_then_get);
    check!(case_put_new_twice_conflicts);
    check!(case_put_wrong_version_conflicts);
    check!(case_lookup_unique);
    check!(case_get_other_owner);
    check!(case_list_owners);
    check!(case_list_filters);
    check!(case_list_pages);
    check!(case_delete_owner);
    check!(case_counter_concurrency);
    check!(case_lease_exclusive);
    check!(case_audit_gapless);
    check!(case_usage_groups);
    failures
}

/// Runs the suite and asserts it is green. The factory must produce a
/// migrated, empty store. For backend tests.
pub async fn assert_green<S: Store>(make: MakeStore<'_, S>) {
    let failures = run(make).await;
    assert!(
        failures.is_empty(),
        "the conformance suite failed: {failures:?}"
    );
}

type CaseResult = std::result::Result<(), &'static str>;

fn now() -> i64 {
    riehub_core::SystemClock.now_millis()
}

fn rec(owner: &Owner, kind: Kind, id: &str) -> Record {
    let mut r = Record::new(owner.clone(), kind, id);
    let t = now();
    r.created_at = t;
    r.updated_at = t;
    r
}

async fn case_put_new_then_get<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let owner = Owner::Instance;
    let r = rec(&owner, Kind::User, "user_test");
    let v = store
        .put(&r, Expect::New)
        .await
        .map_err(|_| "put New failed")?;
    require!(v == 1, "a new record must start at version 1");
    let got = store
        .get(&owner, Kind::User, "user_test")
        .await
        .map_err(|_| "get failed")?;
    let got = got.ok_or("a stored record must come back")?;
    require!(got.id == r.id, "the id must round trip");
    require!(got.kind == Kind::User, "the kind must round trip");
    require!(got.version == 1, "the version must be 1");
    let missing = store
        .get(&owner, Kind::User, "user_none")
        .await
        .map_err(|_| "get failed")?;
    require!(missing.is_none(), "a missing id must be none");
    Ok(())
}

async fn case_put_new_twice_conflicts<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let owner = Owner::Instance;
    let r = rec(&owner, Kind::Session, "sess_once");
    store
        .put(&r, Expect::New)
        .await
        .map_err(|_| "put New failed")?;
    let again = store.put(&r, Expect::New).await;
    require!(again.is_err(), "put New on an existing id must conflict");
    Ok(())
}

async fn case_put_wrong_version_conflicts<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let owner = Owner::Instance;
    let r = rec(&owner, Kind::Connection, "conn_one");
    let v = store
        .put(&r, Expect::New)
        .await
        .map_err(|_| "put New failed")?;
    let mut changed = r.clone();
    changed.updated_at = now();
    let wrong = store.put(&changed, Expect::Version(v + 99)).await;
    require!(wrong.is_err(), "a wrong expected version must conflict");
    let right = store
        .put(&changed, Expect::Version(v))
        .await
        .map_err(|_| "put with the matching version failed")?;
    require!(right == v + 1, "a matching version must bump to the next");
    Ok(())
}

async fn case_lookup_unique<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let user = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap());
    let mut a = rec(&user, Kind::HubKey, "hkey_one");
    a.lookup = Some("alpha".to_owned());
    store
        .put(&a, Expect::New)
        .await
        .map_err(|_| "put with lookup failed")?;
    let mut b = rec(&user, Kind::HubKey, "hkey_two");
    b.lookup = Some("alpha".to_owned());
    let clash = store.put(&b, Expect::New).await;
    require!(
        clash.is_err(),
        "the same lookup on the same owner and kind must conflict"
    );
    let mut other_kind = rec(&user, Kind::Connection, "conn_other");
    other_kind.lookup = Some("alpha".to_owned());
    let ok = store.put(&other_kind, Expect::New).await;
    require!(ok.is_ok(), "the same lookup on another kind must be fine");
    let mut other_owner = rec(&Owner::Instance, Kind::HubKey, "hkey_inst");
    other_owner.lookup = Some("alpha".to_owned());
    let ok = store.put(&other_owner, Expect::New).await;
    require!(ok.is_ok(), "the same lookup on another owner must be fine");
    let by_lookup = store
        .get_by_lookup(&user, Kind::HubKey, "alpha")
        .await
        .map_err(|_| "get_by_lookup failed")?
        .ok_or("the lookup must find the record")?;
    require!(
        by_lookup.id == "hkey_one",
        "the lookup must hit the right record"
    );
    Ok(())
}

async fn case_get_other_owner<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let alice = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap());
    let bob = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap());
    store
        .put(&rec(&alice, Kind::Chat, "chat_private"), Expect::New)
        .await
        .map_err(|_| "put failed")?;
    let seen = store
        .get(&bob, Kind::Chat, "chat_private")
        .await
        .map_err(|_| "get failed")?;
    require!(seen.is_none(), "another owner must never see the record");
    Ok(())
}

async fn case_list_owners<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let alice = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap());
    let bob = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap());
    for id in ["chat_a1", "chat_a2"] {
        store
            .put(&rec(&alice, Kind::Chat, id), Expect::New)
            .await
            .map_err(|_| "put failed")?;
    }
    store
        .put(&rec(&bob, Kind::Chat, "chat_b1"), Expect::New)
        .await
        .map_err(|_| "put failed")?;
    let listed = store
        .list(
            &alice,
            Kind::Chat,
            &Filter::default(),
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list failed")?;
    require!(listed.items.len() == 2, "listing must never cross owners");
    require!(
        listed.items.iter().all(|r| r.id.starts_with("chat_a")),
        "the list must hold only that owner's records"
    );
    Ok(())
}

async fn case_list_filters<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let owner = Owner::Instance;
    for (i, (parent, status)) in [
        ("chat_x", Some("open")),
        ("chat_x", Some("closed")),
        ("chat_x", None),
        ("chat_y", Some("open")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut r = rec(&owner, Kind::ChatMessage, &format!("msg_{i}"));
        r.parent = Some(parent.to_owned());
        r.status = status.map(|s| s.to_owned());
        r.sort = i as i64;
        store.put(&r, Expect::New).await.map_err(|_| "put failed")?;
    }

    let by_parent = store
        .list(
            &owner,
            Kind::ChatMessage,
            &Filter {
                parent: Some("chat_x".to_owned()),
                ..Default::default()
            },
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list with a parent filter failed")?;
    require!(by_parent.items.len() == 3, "the parent filter must hold");

    let by_status = store
        .list(
            &owner,
            Kind::ChatMessage,
            &Filter {
                status: Some("open".to_owned()),
                ..Default::default()
            },
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list with a status filter failed")?;
    require!(by_status.items.len() == 2, "the status filter must hold");

    let by_sort = store
        .list(
            &owner,
            Kind::ChatMessage,
            &Filter {
                sort_min: Some(1),
                sort_max: Some(2),
                ..Default::default()
            },
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list with a sort range failed")?;
    require!(by_sort.items.len() == 2, "the sort range must hold");
    require!(
        by_sort.items.windows(2).all(|w| w[0].sort <= w[1].sort),
        "rows must come back in sort order"
    );
    Ok(())
}

async fn case_list_pages<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let owner = Owner::Instance;
    for i in 0..5i64 {
        let mut r = rec(&owner, Kind::Skill, &format!("skill_{i}"));
        r.sort = i;
        store.put(&r, Expect::New).await.map_err(|_| "put failed")?;
    }
    let mut after = None;
    let mut seen = Vec::new();
    loop {
        let page = store
            .list(
                &owner,
                Kind::Skill,
                &Filter::default(),
                &Page {
                    after: after.clone(),
                    limit: 2,
                },
            )
            .await
            .map_err(|_| "list with a cursor failed")?;
        seen.extend(page.items.iter().map(|r| r.id.clone()));
        match page.next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    require!(
        seen.len() == 5,
        "the cursor must walk every row exactly once"
    );
    let unique = seen.iter().collect::<std::collections::BTreeSet<_>>();
    require!(
        unique.len() == 5,
        "there must be no duplicates across pages"
    );
    Ok(())
}

async fn case_delete_owner<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    let alice = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap());
    let bob = Owner::User(riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap());
    for (owner, kind, id) in [
        (&alice, Kind::Chat, "chat_a1"),
        (&alice, Kind::Session, "sess_a1"),
        (&bob, Kind::Chat, "chat_b1"),
    ] {
        store
            .put(&rec(owner, kind, id), Expect::New)
            .await
            .map_err(|_| "put failed")?;
    }
    let removed = store
        .delete_owner(&alice)
        .await
        .map_err(|_| "delete_owner failed")?;
    require!(removed == 2, "delete_owner must remove exactly that owner");
    let alice_left = store
        .list(
            &alice,
            Kind::Chat,
            &Filter::default(),
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list failed")?;
    require!(
        alice_left.items.is_empty(),
        "the deleted owner must be empty"
    );
    let bob_left = store
        .list(
            &bob,
            Kind::Chat,
            &Filter::default(),
            &Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .map_err(|_| "list failed")?;
    require!(
        bob_left.items.len() == 1,
        "another owner must keep everything"
    );
    Ok(())
}

async fn case_counter_concurrency<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = std::sync::Arc::new(make().await.map_err(|_| "the store factory failed")?);
    store.migrate().await.map_err(|_| "migrate failed")?;
    let key = CounterKey::new(Owner::Instance, "rate:one");
    let window = 1;
    let mut tasks = Vec::new();
    for _ in 0..100 {
        let store = store.clone();
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            store
                .counter_add(&key, window, 1, 9_999_999_999_999)
                .await
                .expect("counter_add must not fail")
        }));
    }
    for task in tasks {
        task.await.expect("the counter task must not panic");
    }
    let total = store
        .counter_get(&key, window)
        .await
        .map_err(|_| "counter_get failed")?;
    require!(total == 100, "counter_add must be atomic under concurrency");
    Ok(())
}

async fn case_lease_exclusive<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    store.migrate().await.map_err(|_| "migrate failed")?;
    let got = store
        .lease_acquire("job", "alice", 60_000)
        .await
        .map_err(|_| "lease_acquire failed")?;
    require!(got, "a free lease must be acquired");
    let got = store
        .lease_acquire("job", "bob", 60_000)
        .await
        .map_err(|_| "lease_acquire failed")?;
    require!(!got, "a held lease must be exclusive");
    store
        .lease_release("job", "alice")
        .await
        .map_err(|_| "lease_release failed")?;
    let got = store
        .lease_acquire("job", "bob", 60_000)
        .await
        .map_err(|_| "lease_acquire failed")?;
    require!(got, "a released lease must be acquirable");
    let got = store
        .lease_acquire("short", "alice", 30)
        .await
        .map_err(|_| "lease_acquire failed")?;
    require!(got, "a short lease must be acquired");
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let got = store
        .lease_acquire("short", "bob", 60_000)
        .await
        .map_err(|_| "lease_acquire failed")?;
    require!(got, "an expired lease must be acquirable");
    Ok(())
}

async fn case_audit_gapless<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = std::sync::Arc::new(make().await.map_err(|_| "the store factory failed")?);
    store.migrate().await.map_err(|_| "migrate failed")?;
    let mac = ConformanceMac;
    let mut tasks = Vec::new();
    for t in 0..10u64 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let events = (0..5u64)
                .map(|i| AuditDraft {
                    ts: now(),
                    kind: "test".to_owned(),
                    event: serde_json::json!({"task": t, "i": i}),
                })
                .collect();
            store
                .audit_append(events, &mac)
                .await
                .expect("audit_append must not fail")
        }));
    }
    let mut heads = Vec::new();
    for task in tasks {
        heads.push(task.await.expect("the audit task must not panic"));
    }
    let head: AuditHead = heads.into_iter().max_by_key(|h| h.seq).unwrap();
    require!(head.seq == 50, "every event must be appended exactly once");

    let rows: Vec<AuditRow> = store
        .audit_read(1, 100, &Default::default())
        .await
        .map_err(|_| "audit_read failed")?;
    require!(rows.len() == 50, "all events must come back");
    for (i, row) in rows.iter().enumerate() {
        require!(
            row.seq == (i + 1) as u64,
            "sequence numbers must be gapless"
        );
    }
    // The chain verifies: recompute the macs from the stored rows.
    let mut prev: Vec<u8> = Vec::new();
    for row in &rows {
        let json = serde_json::to_string(&row.event).unwrap();
        let input = crate::audit::mac_input(&prev, row.seq, row.ts, &row.kind, &json);
        let expected = mac.compute(&prev, &input);
        require!(row.mac == expected.to_vec(), "the chain macs must verify");
        prev = row.mac.clone();
    }
    require!(prev == head.mac, "the stored head must match the last row");
    Ok(())
}

async fn case_usage_groups<S: Store>(make: MakeStore<'_, S>) -> CaseResult {
    let store = make().await.map_err(|_| "the store factory failed")?;
    store.migrate().await.map_err(|_| "migrate failed")?;
    let base = 1_700_000_000_000i64;
    let hour = 3_600_000;
    for (i, (model, status, tokens)) in [
        ("m-one", "ok", 10),
        ("m-one", "ok", 20),
        ("m-one", "error", 5),
        ("m-two", "ok", 7),
    ]
    .into_iter()
    .enumerate()
    {
        let row = UsageRow {
            ts: base + (i % 2) as i64 * hour,
            owner: Owner::Instance,
            user: Some("user_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned()),
            team: None,
            hub_key: None,
            project: None,
            provider_kind: Some("anthropic".to_owned()),
            model: Some(model.to_owned()),
            status: Some(status.to_owned()),
            tokens_in: tokens,
            tokens_out: 0,
            cost_micros: tokens,
        };
        store
            .usage_insert(&row)
            .await
            .map_err(|_| "usage_insert failed")?;
    }

    let q = UsageQuery {
        owner: Owner::Instance,
        from_ts: base,
        to_ts: base + 2 * hour,
        group_by: vec![UsageGroup::Hour, UsageGroup::Model, UsageGroup::Status],
        status: Some("ok".to_owned()),
        model: None,
        user: None,
    };
    let buckets: Vec<UsageBucket> = store
        .usage_query(&q)
        .await
        .map_err(|_| "usage_query failed")?;
    require!(
        buckets.len() == 3,
        "the ok rows must group into three buckets"
    );
    let total_requests: i64 = buckets.iter().map(|b| b.requests).sum();
    require!(
        total_requests == 3,
        "the status filter must leave three rows"
    );
    let one_ok: Vec<&UsageBucket> = buckets
        .iter()
        .filter(|b| b.keys.first() == Some(&"m-one".to_owned()))
        .collect();
    require!(one_ok.len() == 2, "the m-one ok rows span two hour buckets");
    let m_one_tokens: i64 = one_ok.iter().map(|b| b.tokens_in).sum();
    require!(
        m_one_tokens == 30,
        "tokens must sum inside the m-one buckets"
    );

    let by_user = UsageQuery {
        owner: Owner::Instance,
        from_ts: base,
        to_ts: base + 2 * hour,
        group_by: vec![UsageGroup::User],
        status: None,
        model: None,
        user: None,
    };
    let buckets = store
        .usage_query(&by_user)
        .await
        .map_err(|_| "usage_query failed")?;
    require!(buckets.len() == 1, "all rows must group under one user");
    require!(
        buckets[0].requests == 4,
        "the user bucket must hold every row"
    );

    let filtered = UsageQuery {
        owner: Owner::Instance,
        from_ts: base,
        to_ts: base + 2 * hour,
        group_by: vec![UsageGroup::Model],
        status: None,
        model: Some("m-two".to_owned()),
        user: None,
    };
    let buckets = store
        .usage_query(&filtered)
        .await
        .map_err(|_| "usage_query failed")?;
    require!(buckets.len() == 1, "the model filter must leave one bucket");
    require!(buckets[0].requests == 1, "the model filter must work");
    Ok(())
}
