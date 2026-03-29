//! Issue #272 — Archive / restore ownership hardening.
//! Focused security cases; the main suite lives in `lib.rs` under `#[cfg(test)] mod test`.

use crate::{BillPayments, BillPaymentsClient, Error};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};

fn env_with_auth() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn issue272_get_archived_bill_rejects_wrong_caller() {
    let env = env_with_auth();
    let cid = env.register_contract(None, BillPayments);
    let client = BillPaymentsClient::new(&env, &cid);
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);

    let due = env.ledger().timestamp() + 10_000;
    let bill_id = client.create_bill(
        &owner,
        &String::from_str(&env, "ArchTest"),
        &100i128,
        &due,
        &false,
        &0u32,
        &None,
        &String::from_str(&env, "XLM"),
    );
    client.pay_bill(&owner, &bill_id);
    client.archive_paid_bills(&owner, &(env.ledger().timestamp() + 1));

    match client.try_get_archived_bill(&attacker, &bill_id) {
        Err(Ok(Error::Unauthorized)) => {}
        _ => panic!("expected contract error Unauthorized for non-owner get_archived_bill"),
    }
}

#[test]
fn issue272_get_archived_bill_succeeds_for_owner() {
    let env = env_with_auth();
    let cid = env.register_contract(None, BillPayments);
    let client = BillPaymentsClient::new(&env, &cid);
    let owner = Address::generate(&env);

    let due = env.ledger().timestamp() + 10_000;
    let bill_id = client.create_bill(
        &owner,
        &String::from_str(&env, "Owned"),
        &50i128,
        &due,
        &false,
        &0u32,
        &None,
        &String::from_str(&env, "USDC"),
    );
    client.pay_bill(&owner, &bill_id);
    client.archive_paid_bills(&owner, &(env.ledger().timestamp() + 1));

    let ab = client.get_archived_bill(&owner, &bill_id);
    assert_eq!(ab.owner, owner);
    assert_eq!(ab.id, bill_id);
}

#[test]
fn issue272_restore_bill_rejects_non_owner() {
    let env = env_with_auth();
    let cid = env.register_contract(None, BillPayments);
    let client = BillPaymentsClient::new(&env, &cid);
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);

    let due = env.ledger().timestamp() + 10_000;
    let bill_id = client.create_bill(
        &owner,
        &String::from_str(&env, "R1"),
        &100i128,
        &due,
        &false,
        &0u32,
        &None,
        &String::from_str(&env, "XLM"),
    );
    client.pay_bill(&owner, &bill_id);
    client.archive_paid_bills(&owner, &(env.ledger().timestamp() + 1));

    assert_eq!(
        client.try_restore_bill(&attacker, &bill_id),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn issue272_bulk_cleanup_does_not_remove_other_owner_archives() {
    let env = env_with_auth();
    let cid = env.register_contract(None, BillPayments);
    let client = BillPaymentsClient::new(&env, &cid);
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    let due = env.ledger().timestamp() + 20_000;
    let id_a = client.create_bill(
        &a,
        &String::from_str(&env, "A"),
        &10i128,
        &due,
        &false,
        &0u32,
        &None,
        &String::from_str(&env, "XLM"),
    );
    let id_b = client.create_bill(
        &b,
        &String::from_str(&env, "B"),
        &20i128,
        &due,
        &false,
        &0u32,
        &None,
        &String::from_str(&env, "XLM"),
    );
    client.pay_bill(&a, &id_a);
    client.pay_bill(&b, &id_b);

    let ts = env.ledger().timestamp() + 1;
    client.archive_paid_bills(&a, &ts);
    client.archive_paid_bills(&b, &ts);

    let cutoff = env.ledger().timestamp() + 86_400;
    assert_eq!(client.bulk_cleanup_bills(&a, &cutoff), 1);
    let still = client.get_archived_bill(&b, &id_b);
    assert_eq!(still.owner, b);
}
