# Bill Payments Contract

A Soroban smart contract for managing bill payments with support for recurring bills, payment tracking, and access control.

## Overview

The Bill Payments contract allows users to create, manage, and pay bills. It supports both one-time and recurring bills, tracks payment history, and provides comprehensive querying capabilities.

## Features

- Create one-time or recurring bills
- Mark bills as paid with automatic recurring bill generation
- Query unpaid, overdue, and all bills
- Access control ensuring only owners can manage their bills
- Event emission for audit trails
- Storage TTL management for efficiency

## Quickstart

This section provides a minimal example of how to interact with the Bill Payments contract. 

**Gotchas:** 
- The contract uses a paginated API for most list queries natively.
- Bill amounts are specified in the lowest denomination (e.g., stroops for XLM).
- If a bill is marked as `recurring`, paying it automatically generates the next bill.

### Write Example: Creating a Bill
*Note: This is pseudo-code demonstrating the Soroban Rust SDK CLI or client approach.*
```rust

let bill_id = client.create_bill(
    &owner_address,
    &String::from_str(&env, "Internet Bill"),
    &500_0000000,
    &(env.ledger().timestamp() + 2592000),
    &false,
    &0,
    &None,
    &String::from_str(&env, "XLM"),
);

```

### Read Example: Fetching Unpaid Bills
```rust

let limit = 10;
let cursor = 0; 
let page = client.get_unpaid_bills(&owner_address, &cursor, &limit);

```

## API Reference

### Data Structures

#### Bill
```rust
pub struct Bill {
    pub id: u32,
    pub owner: Address,
    pub name: String,
    pub amount: i128,
    pub due_date: u64,
    pub recurring: bool,
    pub frequency_days: u32,
    pub paid: bool,
    pub created_at: u64,
    pub paid_at: Option<u64>,
}
```

#### Error Codes
- `BillNotFound = 1`: Bill with specified ID doesn't exist
- `BillAlreadyPaid = 2`: Attempting to pay an already paid bill
- `InvalidAmount = 3`: Amount is zero or negative
- `InvalidFrequency = 4`: Recurring bill has zero frequency
- `Unauthorized = 5`: Caller is not the bill owner (or not the archived row owner where required)
- `InconsistentBillData = 15`: Map key and embedded `id` disagree, or restore would collide with inconsistent active state (see Archive and restore)

### Functions

#### `create_bill(env, owner, name, amount, due_date, recurring, frequency_days) -> Result<u32, Error>`
Creates a new bill.

**Parameters:**
- `owner`: Address of the bill owner (must authorize)
- `name`: Bill name (e.g., "Electricity", "School Fees")
- `amount`: Payment amount (must be positive)
- `due_date`: Due date as Unix timestamp
- `recurring`: Whether this is a recurring bill
- `frequency_days`: Frequency in days for recurring bills (> 0 if recurring)

**Returns:** Bill ID on success

**Errors:** InvalidAmount, InvalidFrequency

#### `pay_bill(env, caller, bill_id) -> Result<(), Error>`
Marks a bill as paid.

**Parameters:**
- `caller`: Address of the caller (must be bill owner)
- `bill_id`: ID of the bill to pay

**Returns:** Ok(()) on success

**Errors:** BillNotFound, BillAlreadyPaid, Unauthorized

#### `get_bill(env, bill_id) -> Option<Bill>`
Retrieves a bill by ID.

**Parameters:**
- `bill_id`: ID of the bill

**Returns:** Bill struct or None if not found

#### `get_unpaid_bills(env, owner) -> Vec<Bill>`
Gets all unpaid bills for an owner.

**Parameters:**
- `owner`: Address of the bill owner

**Returns:** Vector of unpaid Bill structs

#### `get_overdue_bills(env, owner) -> Vec<Bill>`
Gets all overdue unpaid bills for a specific owner.

**Parameters:**
- `owner`: Address of the bill owner

**Returns:** Vector of overdue Bill structs belonging to the owner

#### `get_total_unpaid(env, owner) -> i128`
Calculates total amount of unpaid bills for an owner.

**Parameters:**
- `owner`: Address of the bill owner

**Returns:** Total unpaid amount

#### `cancel_bill(env, bill_id) -> Result<(), Error>`
Cancels/deletes a bill.

**Parameters:**
- `bill_id`: ID of the bill to cancel

**Returns:** Ok(()) on success

**Errors:** BillNotFound

#### `get_all_bills(env) -> Vec<Bill>`
Gets all bills (paid and unpaid).

**Returns:** Vector of all Bill structs

## Usage Examples

### Creating a One-Time Bill
```rust
// Create a one-time electricity bill due in 30 days
let bill_id = bill_payments::create_bill(
    env,
    user_address,
    "Electricity Bill".into(),
    150_0000000, // 150 XLM in stroops
    env.ledger().timestamp() + (30 * 86400), // 30 days from now
    false, // not recurring
    0, // frequency not needed
)?;
```

### Creating a Recurring Bill
```rust
// Create a monthly insurance bill
let bill_id = bill_payments::create_bill(
    env,
    user_address,
    "Insurance Premium".into(),
    50_0000000, // 50 XLM
    env.ledger().timestamp() + (30 * 86400), // due in 30 days
    true, // recurring
    30, // every 30 days
)?;
```

### Paying a Bill
```rust
// Pay the bill (caller must be the owner)
bill_payments::pay_bill(env, user_address, bill_id)?;
```

### Querying Bills
```rust
// Get all unpaid bills for a user
let unpaid = bill_payments::get_unpaid_bills(env, user_address);

// Get total unpaid amount
let total = bill_payments::get_total_unpaid(env, user_address);

// Check for overdue bills
let overdue = bill_payments::get_overdue_bills(env, user_address);
```

## Events

The contract emits events for audit trails:
- `BillEvent::Created`: When a bill is created
- `BillEvent::Paid`: When a bill is paid

## Integration Patterns

### With Remittance Split
The bill payments contract integrates with the remittance split contract to automatically allocate funds to bill payments:

```rust
// Calculate split amounts
let split_amounts = remittance_split::calculate_split(env, total_remittance);

// Allocate to bills
let bills_allocation = split_amounts.get(2).unwrap(); // bills percentage

// Create bill payment entries based on allocation
```

### With Insurance Contract
Bills can represent insurance premiums, working alongside the insurance contract for comprehensive financial management.

## Archive and restore (Issue #272)

Paid bills can be moved to **archived** storage, restored to active storage, or **cleaned up** (permanently removed from the archive). These flows enforce **owner affinity**: only the recorded bill owner can mutate or read sensitive archive data for their rows.

### Functions

| Function | Authorization | Notes |
|----------|----------------|--------|
| `archive_paid_bills(caller, before_timestamp)` | `caller.require_auth()` | Archives only rows where `bill.owner == caller`, `bill.paid`, `paid_at < before_timestamp`, and **map key `id` matches `bill.id`** (integrity). |
| `restore_bill(caller, bill_id)` | `caller.require_auth()` | Requires archived row exists, `ArchivedBill.owner == caller`, **`ArchivedBill.id == bill_id`**, and no conflicting active bill at `bill_id`. |
| `bulk_cleanup_bills(caller, before_timestamp)` | `caller.require_auth()` | Deletes only archived rows with `owner == caller`, `archived_at < before_timestamp`, and **`id` key matches `ArchivedBill.id`**. |
| `get_archived_bills(owner, cursor, limit)` | **`owner.require_auth()`** | Listing is not anonymous; the `owner` address must sign. |
| `get_archived_bill(caller, bill_id)` | **`caller.require_auth()`** | Returns `Ok(archived)` only if the archived row exists **and** `archived.owner == caller` and ids are consistent; otherwise `BillNotFound`, `Unauthorized`, or `InconsistentBillData`. |

### Error code

- **`InconsistentBillData = 15`**: Thrown when a stored `Bill` / `ArchivedBill` has an `id` field that does not match its map key, when restoring would collide with an existing active row for the same id, or when cleanup detects the same mismatch. This blocks silent cross-owner or corrupted-key exploitation.

### NatSpec-style notes (contract source)

The contract uses `@notice`, `@param`, `@return`, `@dev`, and `@errors` on archive-related entrypoints in `bill_payments/src/lib.rs` for reviewers and tooling.

### Client migration

- **`get_archived_bill`**: Signature is now `(env, caller, bill_id) -> Result<ArchivedBill, Error>` (previously a public `Option` by `bill_id` only). Callers must pass the **authenticated owner** as `caller`.

## Security considerations

- **Authorization**: Mutating functions use `require_auth()` on the acting address; archive listings require the **same** owner address to authorize reads.
- **Owner affinity**: Archive, restore, and cleanup iterate or select rows **only** for `caller` / authenticated `owner`; cross-owner manipulation returns `Unauthorized` or fails integrity checks.
- **Consistency**: Key/id checks reduce the risk of inconsistent maps being used to move or read another user’s data.
- **Input validation**: Amounts, due dates, and recurrence rules are validated on create/update paths.
- **Storage TTL**: Instance and archive TTL bumps keep data available without unbounded retention of abandoned state.

### Validation (tests)

Run from the workspace root:

```bash
cargo test -p bill_payments
```

All `bill_payments` unit tests, integration tests under `bill_payments/tests/`, and Issue #272 cases in `bill_payments/src/test.rs` should pass. Focused security tests cover: non-owner `get_archived_bill` / `restore_bill`, and `bulk_cleanup` not removing another owner’s archived rows.

**Security assumptions**

- Stellar account signatures are unforgeable; `require_auth()` correctly binds the caller to the intended `Address`.
- Archive state is only produced through `archive_paid_bills` (paid bills, owner-checked); callers should treat `InconsistentBillData` as a fatal integrity signal and investigate off-chain.