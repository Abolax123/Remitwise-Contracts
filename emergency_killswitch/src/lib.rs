#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, Address, Env, Symbol};

// ---------------------------------------------------------------------------
// Storage Keys
// ---------------------------------------------------------------------------

const KEY_ADMIN: Symbol = symbol_short!("ADMIN");
const KEY_PAUSED: Symbol = symbol_short!("PAUSED");
const KEY_UNP_AT: Symbol = symbol_short!("UNP_AT");
const KEY_LAST_PAUSE: Symbol = symbol_short!("LST_PAU");
const KEY_RESOLVED: Symbol = symbol_short!("RESOLVED");

const MIN_COOLDOWN: u64 = 3600; // 1 hour in seconds

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `initialize` was never called.
    NotInitialized = 1,
    /// `initialize` has already been called.
    AlreadyInitialized = 2,
    /// Caller is not the current admin.
    Unauthorized = 3,
    /// The contract (or target function) is paused.
    ContractPaused = 4,
    /// Scheduled-unpause timestamp must be in the future.
    InvalidSchedule = 5,
    /// Minimum cooldown period has not yet elapsed.
    UnderCooldown = 6,
    /// The emergency state has not been marked as resolved.
    NotResolved = 7,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Thin helper – keeps event topic construction in one place.
fn emit(env: &Env, action: Symbol) {
    env.events()
        .publish((symbol_short!("killswtch"), action), ());
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct EmergencyKillswitch;

#[contractimpl]
impl EmergencyKillswitch {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// One-shot setup. Stores `admin` and leaves the contract unpaused.
    /// Fails with `AlreadyInitialized` if called a second time.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env
            .storage()
            .instance()
            .get::<_, Address>(&KEY_ADMIN)
            .is_some()
        {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&KEY_ADMIN, &admin);
        env.storage().instance().set(&KEY_PAUSED, &false);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&KEY_ADMIN)
            .ok_or(Error::NotInitialized)
    }

    fn check_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin = Self::admin(env)?;
        if admin != *caller {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), Error> {
        let paused: bool = env.storage().instance().get(&KEY_PAUSED).unwrap_or(false);
        if paused {
            Err(Error::ContractPaused)
        } else {
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Pause controls
    // -----------------------------------------------------------------------

    /// Pause the contract. Only the admin may call this.
    /// Emits a `"paused"` event.
    pub fn pause(env: Env, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        Self::check_admin(&env, &caller)?;
        env.storage().instance().set(&KEY_PAUSED, &true);
        env.storage()
            .instance()
            .set(&KEY_LAST_PAUSE, &env.ledger().timestamp());
        env.storage().instance().set(&KEY_RESOLVED, &false);
        emit(&env, symbol_short!("paused"));
        Ok(())
    }

    /// Mark the underlying emergency as resolved. This is a prerequisite for unpausing.
    /// Only the admin may call this.
    pub fn mark_resolved(env: Env, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        Self::check_admin(&env, &caller)?;
        env.storage().instance().set(&KEY_RESOLVED, &true);
        emit(&env, symbol_short!("resolved"));
        Ok(())
    }

    /// Unpause the contract. Only the admin may call this.
    ///
    /// Safety Checks:
    /// 1. Minimum cooldown (`MIN_COOLDOWN`) must have elapsed since the last pause.
    /// 2. The incident must be explicitly marked as `resolved`.
    /// 3. Any manually scheduled delay (`schedule_unpause`) must have passed.
    pub fn unpause(env: Env, caller: Address) -> Result<(), Error> {
        caller.require_auth();
        Self::check_admin(&env, &caller)?;

        // 1. Mandatory Cooldown Check
        let last_pause: u64 = env.storage().instance().get(&KEY_LAST_PAUSE).unwrap_or(0);
        if env.ledger().timestamp() < last_pause + MIN_COOLDOWN {
            return Err(Error::UnderCooldown);
        }

        // 2. Resolution Check
        let resolved: bool = env.storage().instance().get(&KEY_RESOLVED).unwrap_or(false);
        if !resolved {
            return Err(Error::NotResolved);
        }

        // 3. Manual Schedule Check
        let unp_at: Option<u64> = env.storage().instance().get(&KEY_UNP_AT);
        if let Some(at) = unp_at {
            if env.ledger().timestamp() < at {
                return Err(Error::ContractPaused);
            }
            env.storage().instance().remove(&KEY_UNP_AT);
        }

        env.storage().instance().set(&KEY_PAUSED, &false);
        env.storage().instance().remove(&KEY_RESOLVED);
        env.storage().instance().remove(&KEY_LAST_PAUSE);
        emit(&env, symbol_short!("unpaused"));
        Ok(())
    }

    /// Set a future timestamp before which `unpause` will be rejected.
    /// This gives operators a mandatory cooling-off period after an incident.
    pub fn schedule_unpause(env: Env, caller: Address, at_timestamp: u64) -> Result<(), Error> {
        caller.require_auth();
        Self::check_admin(&env, &caller)?;

        if at_timestamp <= env.ledger().timestamp() {
            return Err(Error::InvalidSchedule);
        }
        env.storage().instance().set(&KEY_UNP_AT, &at_timestamp);
        Ok(())
    }

    /// Transfer admin rights to `new_admin`. Only the current admin may call
    /// this. The new admin takes effect immediately.
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), Error> {
        caller.require_auth();
        Self::check_admin(&env, &caller)?;
        env.storage().instance().set(&KEY_ADMIN, &new_admin);
        emit(&env, symbol_short!("adm_xfr"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Representative guarded operations
    // (demonstrate the pause-check pattern; no real token movement)
    // -----------------------------------------------------------------------

    /// Simulated mutating transfer — blocked while paused.
    pub fn do_transfer(env: Env, caller: Address, _amount: i128) -> Result<(), Error> {
        caller.require_auth();
        Self::assert_not_paused(&env)?;
        // Real implementation would move tokens here.
        Ok(())
    }

    /// Simulated mint — blocked while paused.
    pub fn do_mint(env: Env, caller: Address, _amount: i128) -> Result<(), Error> {
        caller.require_auth();
        Self::assert_not_paused(&env)?;
        // Real implementation would mint tokens here.
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read-only queries (always available, even while paused)
    // -----------------------------------------------------------------------

    /// Returns `true` when the contract is globally paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&KEY_PAUSED).unwrap_or(false)
    }

    /// Returns the current admin address, or `None` if not yet initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&KEY_ADMIN)
    }

    /// Returns the scheduled-unpause timestamp, if one has been set.
    pub fn get_scheduled_unpause(env: Env) -> Option<u64> {
        env.storage().instance().get(&KEY_UNP_AT)
    }
}
