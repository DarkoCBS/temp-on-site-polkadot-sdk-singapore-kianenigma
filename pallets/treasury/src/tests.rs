use crate::{mock::*, *};
use frame_support::assert_err;
use frame_support::traits::fungible::Inspect;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok, traits::fungibles};
use sp_io::TestExternalities as TestState;
use sp_runtime::traits::BadOrigin;
use sp_runtime::BoundedVec;
use crate::tests::AmountHeldOnProposal;

pub(crate) const ALICE: u64 = 1;
pub(crate) const BOB: u64 = 2;
pub(crate) const CHARLIE: u64 = 3;

pub(crate) struct StateBuilder {
	balances: Vec<(<Test as frame_system::Config>::AccountId, Balance)>,
}

impl Default for StateBuilder {
	fn default() -> Self {
		let treasury_account_id = Treasury::treasury_account_id();

		Self {
			balances: vec![(treasury_account_id, 999_999), (ALICE, 100_000), (BOB, 100_000)],
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

			System::set_block_number(1);
		});

		ext.execute_with(test);

		// Assertions that must always hold (system invariants)
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
fn fund_treasury_asset() {
	StateBuilder::default().build_and_execute(|| {
		// Check initial treasury balance
		let treasury_account = &Treasury::treasury_account_id();
		assert_eq!(<Test as Config>::NativeBalance::balance(treasury_account), 999_999);

		// Fund Treasury
		let fund_treasury_amount = 1;
		assert_ok!(Treasury::fund_treasury_native(
			RuntimeOrigin::signed(ALICE),
			fund_treasury_amount
		));

		// Check Treasury balance after funding
		assert_eq!(
			<Test as Config>::NativeBalance::balance(treasury_account),
			999_999 + fund_treasury_amount
		);
		// Check Alice balance after funding
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			100_000 - fund_treasury_amount
		);
	});
}

#[test]
fn spending_proposal_instant_payout() {
	StateBuilder::default().build_and_execute(|| {
		// Check pre state
		assert!(SpendingProposals::<Test>::get(ALICE, 0).is_none());

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			123_000,
			ALICE,
			ALICE,
			PayoutType::Instant
		));

		System::assert_last_event(Event::AddedProposal { proposer: ALICE, index_count: 0, amount: 123_000, title: BoundedVec::truncate_from("Title".as_bytes().into()) }.into() );

		// Check post state
		assert!(SpendingProposals::<Test>::get(ALICE, 0).is_some());
	})
}

#[test]
fn approve_proposal_instant_payout() {
	StateBuilder::default().build_and_execute(|| {
		// Check Alice init balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 100_000);

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			123_000,
			ALICE,
			ALICE,
			PayoutType::Instant
		));

		System::assert_last_event(Event::AddedProposal { proposer: ALICE, index_count: 0, amount: 123_000, title: BoundedVec::truncate_from("Title".as_bytes().into()) }.into() );

		// Check pre state
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		let governance_origin = GovernanceOrigin::get();

		// Approve proposal
		assert_ok!(Treasury::approve_proposal(RuntimeOrigin::signed(governance_origin), ALICE, 0));

		// Check Alice post balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 223_000);
	})
}

#[test]
fn approve_proposal_periodic_payout() {
	StateBuilder::default().build_and_execute(|| {
		// Check alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 100_000);

		let periodic_payout = PayoutType::Periodic(PeriodicPayoutPercentage {
			upfront: 20,
			after_fully_complete: 0,
			periodic: 80,
			num_of_periodic_payouts: NumOfPeriodicPayouts::Ten,
			payment_each_n_blocks: 10,
		});

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			100_000,
			ALICE,
			ALICE,
			periodic_payout
		));

		System::assert_last_event(Event::AddedProposal { proposer: ALICE, index_count: 0, amount: 100_000, title: BoundedVec::truncate_from("Title".as_bytes().into()) }.into() );

		// Check if proposal is stored
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		let governance_origin = GovernanceOrigin::get();

		// Approve proposal
		assert_ok!(Treasury::approve_proposal(RuntimeOrigin::signed(governance_origin), ALICE, 0));

		// Check upfront payment
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 120_000);

		let initial_block_number = System::block_number();
		for i in (0..100u128).step_by(10) {
			// Fast forward 10 blocks
			let block_number: u64 =  initial_block_number + i as u64;
			System::set_block_number(block_number);
			Treasury::on_initialize(block_number);

			// Check periodic payment
			let payment_instance_counter = i / 10 + 1;
			let expected_balance = 120_000 + 8_000 * (payment_instance_counter);
			assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), expected_balance);
		}

		// Check Alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 200_000);
	})
}

#[test]
fn do_propose_spend_wrong_payout_type(){
	StateBuilder::default().build_and_execute(|| {
		// Check alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 100_000);

		let periodic_payout = PayoutType::Periodic(PeriodicPayoutPercentage {
			upfront: 100,
			after_fully_complete: 0,
			periodic: 80,
			num_of_periodic_payouts: NumOfPeriodicPayouts::Ten,
			payment_each_n_blocks: 10,
		});

		// Propose spend
		assert_err!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			100_000,
			ALICE,
			ALICE,
			periodic_payout
		), Error::<Test>::PayoutPercentagesMustSumTo100);

		assert_eq!(System::events().len(), 0);

	})
}

#[test]
fn approve_proposal_bad_origin() {
	StateBuilder::default().build_and_execute(|| {
		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			123_000,
			ALICE,
			ALICE,
			PayoutType::Instant
		));

		// Check pre state
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		// Approve proposal with bad origin raises error
		assert_err!(Treasury::approve_proposal(RuntimeOrigin::signed(BOB), ALICE, 0), BadOrigin);
	})
}

// #[test]
// fn it_works_for_default_value() {
// 	StateBuilder::default().build_and_execute(|| {
// 		// Go past genesis block so events get deposited
// 		System::set_block_number(1);
// 		// Dispatch a signed extrinsic.
// 		assert_ok!(Treasury::do_something(RuntimeOrigin::signed(1), 42));
// 		// Read pallet storage and assert an expected result.
// 		// assert_eq!(Something::<Test>::get(), Some(42));
// 		// Assert that the correct event was deposited
// 		System::assert_last_event(Event::SomethingStored { something: 42, who: 1 }.into());
// 	});
// }

// #[test]
// fn correct_error_for_none_value() {
// 	StateBuilder::default().build_and_execute(|| {
// 		// Ensure the expected error is thrown when no value is present.
// 		// assert_noop!(Treasury::cause_error(RuntimeOrigin::signed(1)), Error::<Test>::NoneValue);
// 	});
// }

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
		assert_eq!(<Test as Config>::Fungibles::balance(asset_id, &alice), <Test as pallet::Config>::AmountHeldOnProposal::get());
	});
}
