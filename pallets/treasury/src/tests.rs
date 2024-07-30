use crate::{mock::*, *};
use frame_support::traits::fungible::Mutate;
use frame_support::{assert_noop, assert_ok, traits::fungibles};
use sp_io::TestExternalities as TestState;

pub(crate) const ALICE: u64 = 1;
pub(crate) const BOB: u64 = 2;
pub(crate) const CHARLIE: u64 = 3;

pub(crate) struct StateBuilder {
	balances: Vec<(<Test as frame_system::Config>::AccountId, Balance)>,
}

impl Default for StateBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(Treasury::treasury_account_id(), 999_999),
				(ALICE, 100_000),
				(BOB, 100_000),
			],
		}
	}
}

impl StateBuilder {
	pub(crate) fn build_and_execute(self, test: impl FnOnce() -> ()) {
		let mut ext = TestState::new_empty();

		// Setup initial state
		ext.execute_with(|| {
			for (who, amount) in &self.balances {
				<Test as Config>::NativeBalance::set_balance(who, *amount);
			}
		});

		ext.execute_with(test);

		// Assertions that must always hold
		ext.execute_with(|| {
			assert_eq!(true, true);
		})
	}

	fn add_user_balance(
		mut self,
		who: <Test as frame_system::Config>::AccountId,
		amount: Balance,
	) -> Self {
		self.balances.push((who, amount));
		self
	}

	fn add_treasury_balance(mut self, amount: Balance) -> Self {
		let treasury_account = Treasury::treasury_account_id();
		// println!("Treasury_account: {:?}", treasury_account);
		// <Test as Config>::NativeBalance::set_balance(treasury_account, amount);
		self.balances.push((treasury_account, amount));
		self
	}
}

#[test]
fn it_works_for_default_value() {
	StateBuilder::default().build_and_execute(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		// Dispatch a signed extrinsic.
		assert_ok!(Treasury::do_something(RuntimeOrigin::signed(1), 42));
		// Read pallet storage and assert an expected result.
		// assert_eq!(Something::<Test>::get(), Some(42));
		// Assert that the correct event was deposited
		System::assert_last_event(Event::SomethingStored { something: 42, who: 1 }.into());
	});
}

#[test]
fn correct_error_for_none_value() {
	StateBuilder::default().build_and_execute(|| {
		// Ensure the expected error is thrown when no value is present.
		// assert_noop!(Treasury::cause_error(RuntimeOrigin::signed(1)), Error::<Test>::NoneValue);
	});
}

#[test]
fn handle_assets() {
	StateBuilder::default().build_and_execute(|| {
		let alice = 1;
		let asset_id = 1337;

		// These are some easy configuration you can use when creating a new token...
		// Don't worry too much about the details here, just know this works.
		let admin = 0;
		let is_sufficient = true;
		let min_balance = 1;

		// Here we show that alice initially does not have any balance of some random asset... as
		// expected.
		assert_eq!(<Test as Config>::Fungibles::balance(asset_id, &alice), 0);

		// Before we can give alice any asset, we must first CREATE that asset in our system. Think
		// about this similar to someone launching a contract on Ethereum. Before they launch the
		// contract, there is no token. For tests, we assume people have created other tokens like
		// BTC, USDC, etc...
		assert_ok!(<<Test as Config>::Fungibles as fungibles::Create<_>>::create(
			asset_id,
			admin,
			is_sufficient,
			min_balance
		));

		// Now that the asset is created, we can mint some balance into the alice account
		assert_ok!(<<Test as Config>::Fungibles as fungibles::Mutate<_>>::mint_into(
			asset_id, &alice, 100
		));

		// And here we can see that alice has this balance.
		assert_eq!(<Test as Config>::Fungibles::balance(asset_id, &alice), 100);
	});
}
