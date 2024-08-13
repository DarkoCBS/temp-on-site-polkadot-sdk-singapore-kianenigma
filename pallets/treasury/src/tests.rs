use crate::{mock::*, *};
use frame_support::assert_err;
use frame_support::traits::fungible::Inspect;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Hooks;
use frame_support::{assert_ok, traits::fungibles};
use sp_io::TestExternalities as TestState;
use sp_runtime::traits::BadOrigin;
use sp_runtime::BoundedVec;
use sp_runtime::Percent;

// Constants for test accounts
pub(crate) const ALICE: u64 = 1;
pub(crate) const BOB: u64 = 2;

const TREASURY_INITIAL_BALANCE: Balance = 999_999;
const ALICE_INITIAL_BALANCE: Balance = 100_000;
const BOB_INITIAL_BALANCE: Balance = 500_000;

const FUND_TREASURY_AMOUNT: Balance = 1;

// StateBuilder struct to manage initial state
pub(crate) struct StateBuilder {
	balances: Vec<(<Test as frame_system::Config>::AccountId, Balance)>,
}

impl Default for StateBuilder {
	fn default() -> Self {
		let treasury_account_id = Treasury::treasury_account_id();

		Self {
			balances: vec![
				(treasury_account_id, TREASURY_INITIAL_BALANCE),
				(ALICE, ALICE_INITIAL_BALANCE),
				(BOB, BOB_INITIAL_BALANCE),
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

	fn set_treasury_balance(mut self, _amount: Balance) -> Self {
		let treasury_account = Treasury::treasury_account_id();
		for (who, amount) in &mut self.balances {
			if who == &treasury_account {
				*amount = _amount;
			}
		}
		self
	}
}

#[test]
fn fund_treasury_asset() {
	StateBuilder::default().build_and_execute(|| {
		// Check initial treasury balance
		let treasury_account = &Treasury::treasury_account_id();
		assert_eq!(
			<Test as Config>::NativeBalance::balance(treasury_account),
			TREASURY_INITIAL_BALANCE
		);

		// Fund Treasury
		assert_ok!(Treasury::fund_treasury_native(
			RuntimeOrigin::signed(ALICE),
			FUND_TREASURY_AMOUNT
		));

		// Check Treasury balance after funding
		assert_eq!(
			<Test as Config>::NativeBalance::balance(treasury_account),
			TREASURY_INITIAL_BALANCE + FUND_TREASURY_AMOUNT
		);
		// Check Alice balance after funding
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			ALICE_INITIAL_BALANCE - FUND_TREASURY_AMOUNT
		);
	});
}

#[test]
fn spending_proposal_instant_payout() {
	StateBuilder::default().build_and_execute(|| {
		let PROPOSAL_AMOUNT = 123_000;

		// Check pre state
		assert!(SpendingProposals::<Test>::get(ALICE, 0).is_none());

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			PROPOSAL_AMOUNT,
			ALICE,
			ALICE,
			PayoutType::Instant
		));

		System::assert_last_event(
			Event::AddedProposal {
				proposer: ALICE,
				index_count: 0,
				amount: PROPOSAL_AMOUNT,
				title: BoundedVec::truncate_from("Title".as_bytes().into()),
			}
			.into(),
		);

		// Check post state
		assert!(SpendingProposals::<Test>::get(ALICE, 0).is_some());
	})
}

#[test]
fn approve_proposal_instant_payout() {
	let PROPOSAL_AMOUNT = 123_000;

	StateBuilder::default().build_and_execute(|| {
		// Check Alice init balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), ALICE_INITIAL_BALANCE);

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			PROPOSAL_AMOUNT,
			ALICE,
			ALICE,
			PayoutType::Instant
		));

		System::assert_last_event(
			Event::AddedProposal {
				proposer: ALICE,
				index_count: 0,
				amount: PROPOSAL_AMOUNT,
				title: BoundedVec::truncate_from("Title".as_bytes().into()),
			}
			.into(),
		);

		// Check pre state
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		let governance_origin = GovernanceOrigin::get();

		// Approve proposal
		assert_ok!(Treasury::approve_proposal(RuntimeOrigin::signed(governance_origin), ALICE, 0));

		// Check Alice post balance
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			ALICE_INITIAL_BALANCE + PROPOSAL_AMOUNT
		);
	})
}

#[test]
fn approve_proposal_periodic_payout() {
	StateBuilder::default().build_and_execute(|| {
		let PROPOSAL_AMOUNT = 100_000;

		// Check alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), ALICE_INITIAL_BALANCE);

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
			PROPOSAL_AMOUNT,
			ALICE,
			ALICE,
			periodic_payout
		));

		System::assert_last_event(
			Event::AddedProposal {
				proposer: ALICE,
				index_count: 0,
				amount: PROPOSAL_AMOUNT,
				title: BoundedVec::truncate_from("Title".as_bytes().into()),
			}
			.into(),
		);

		// Check if proposal is stored
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		let governance_origin = GovernanceOrigin::get();

		// Approve proposal
		assert_ok!(Treasury::approve_proposal(RuntimeOrigin::signed(governance_origin), ALICE, 0));

		// Check upfront payment
		let expected_balance_after_upfront = ALICE_INITIAL_BALANCE + 20_000;
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			expected_balance_after_upfront
		);

		let initial_block_number = System::block_number();
		for i in (10..=100u128).step_by(10) {
			// Fast forward 10 blocks
			let block_number: u64 = initial_block_number + i as u64;
			System::set_block_number(block_number);
			Treasury::on_initialize(block_number);

			// Check periodic payment
			let payment_instance_counter = i / 10;
			let expected_balance =
				expected_balance_after_upfront + 8_000 * (payment_instance_counter);
			assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), expected_balance);
		}

		// Check Alice balance
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			ALICE_INITIAL_BALANCE + PROPOSAL_AMOUNT
		);
	})
}

#[test]
fn do_propose_spend_wrong_payout_type() {
	StateBuilder::default().build_and_execute(|| {
		let PROPOSAL_AMOUNT = 100_000;

		// Check alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), ALICE_INITIAL_BALANCE);

		let periodic_payout = PayoutType::Periodic(PeriodicPayoutPercentage {
			upfront: 100,
			after_fully_complete: 0,
			periodic: 80,
			num_of_periodic_payouts: NumOfPeriodicPayouts::Ten,
			payment_each_n_blocks: 10,
		});

		// Propose spend
		assert_err!(
			Treasury::propose_spend(
				RuntimeOrigin::signed(ALICE),
				BoundedVec::truncate_from("Title".as_bytes().into()),
				BoundedVec::truncate_from("Description".as_bytes().to_vec()),
				0,
				PROPOSAL_AMOUNT,
				ALICE,
				ALICE,
				periodic_payout
			),
			Error::<Test>::PayoutPercentagesMustSumTo100
		);

		assert_eq!(System::events().len(), 0);
	})
}

#[test]
fn approve_proposal_bad_origin() {
	StateBuilder::default().build_and_execute(|| {
		let PROPOSAL_AMOUNT = 123_000;

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			PROPOSAL_AMOUNT,
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

#[test]
pub fn payout_moved_forward() {
	StateBuilder::default().build_and_execute(|| {
		const PROPOSAL_AMOUNT: u128 = 1_000_000;

		// Check alice balance
		assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), ALICE_INITIAL_BALANCE);
		// Check treasury balance
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
			TREASURY_INITIAL_BALANCE
		);

		let periodic_payout = PayoutType::Periodic(PeriodicPayoutPercentage {
			upfront: 50,
			after_fully_complete: 0,
			periodic: 50,
			num_of_periodic_payouts: NumOfPeriodicPayouts::Ten,
			payment_each_n_blocks: 10,
		});

		// Propose spend
		assert_ok!(Treasury::propose_spend(
			RuntimeOrigin::signed(ALICE),
			BoundedVec::truncate_from("Title".as_bytes().into()),
			BoundedVec::truncate_from("Description".as_bytes().to_vec()),
			0,
			PROPOSAL_AMOUNT,
			ALICE,
			ALICE,
			periodic_payout
		));

		System::assert_last_event(
			Event::AddedProposal {
				proposer: ALICE,
				index_count: 0,
				amount: PROPOSAL_AMOUNT,
				title: BoundedVec::truncate_from("Title".as_bytes().into()),
			}
			.into(),
		);

		// Check if proposal is stored
		assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

		let governance_origin = GovernanceOrigin::get();

		// Approve proposal
		assert_ok!(Treasury::approve_proposal(RuntimeOrigin::signed(governance_origin), ALICE, 0));

		// Check upfront payment
		let expected_balance_after_upfront = ALICE_INITIAL_BALANCE + 500_000;
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			expected_balance_after_upfront
		);

		let initial_block_number = System::block_number();
		let mut block_number = initial_block_number;
		const AMOUNT_PER_PERIODIC_PAYMENT: u128 = 50_000;
		for i in (10..=90u128).step_by(10) {
			// Fast forward 10 blocks
			block_number = initial_block_number + i as u64;
			System::set_block_number(block_number);
			Treasury::on_initialize(block_number);

			// Check periodic payment

			let payment_instance_counter = i / 10;
			let expected_balance = expected_balance_after_upfront
				+ AMOUNT_PER_PERIODIC_PAYMENT * (payment_instance_counter);
			assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), expected_balance);
		}

		System::set_block_number(block_number + 10);
		Treasury::on_initialize(block_number + 10);

		System::assert_last_event(
			Event::PayoutMovedForward {
				curr_block_number: 101,
				moved_to_block_number: 111,
				proposer: ALICE,
				beneficiary: ALICE,
				asset_id: 0,
				amount: AMOUNT_PER_PERIODIC_PAYMENT,
			}
			.into(),
		);

		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			ALICE_INITIAL_BALANCE + PROPOSAL_AMOUNT - AMOUNT_PER_PERIODIC_PAYMENT
		);

		// Check Treasury balance
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
			TREASURY_INITIAL_BALANCE - (PROPOSAL_AMOUNT - AMOUNT_PER_PERIODIC_PAYMENT)
		);

		// Fund Treasury to be able to send last payment
		assert_ok!(Treasury::fund_treasury_native(
			RuntimeOrigin::signed(BOB),
			FUND_TREASURY_AMOUNT
		));

		System::set_block_number(block_number + 20);
		Treasury::on_initialize(block_number + 20);

		// Assert that the last payment was sent
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&ALICE),
			PROPOSAL_AMOUNT + ALICE_INITIAL_BALANCE
		);
	});
}

#[test]
fn periodic_payout_complex_case() {
	const TREASURY_INITIAL_BALANCE: Balance = 799_999;

	StateBuilder::default()
		.set_treasury_balance(TREASURY_INITIAL_BALANCE)
		.build_and_execute(|| {
			const PROPOSAL_AMOUNT: u128 = 1_000_000;

			// Check alice balance
			assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), ALICE_INITIAL_BALANCE);
			// Check treasury balance
			assert_eq!(
				<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
				TREASURY_INITIAL_BALANCE
			);

			let periodic_payout = PayoutType::Periodic(PeriodicPayoutPercentage {
				upfront: 40,
				after_fully_complete: 20,
				periodic: 40,
				num_of_periodic_payouts: NumOfPeriodicPayouts::Ten,
				payment_each_n_blocks: 10,
			});

			let AFTER_FULLY_COMPLETE_AMOUNT: u128 = Percent::from_percent(20) * PROPOSAL_AMOUNT;

			// Propose spend
			assert_ok!(Treasury::propose_spend(
				RuntimeOrigin::signed(ALICE),
				BoundedVec::truncate_from("Title".as_bytes().into()),
				BoundedVec::truncate_from("Description".as_bytes().to_vec()),
				0,
				PROPOSAL_AMOUNT,
				ALICE,
				ALICE,
				periodic_payout
			));

			System::assert_last_event(
				Event::AddedProposal {
					proposer: ALICE,
					index_count: 0,
					amount: PROPOSAL_AMOUNT,
					title: BoundedVec::truncate_from("Title".as_bytes().into()),
				}
				.into(),
			);

			// Check if proposal is stored
			assert_eq!(SpendingProposals::<Test>::get(ALICE, 0).unwrap().approved, false);

			let governance_origin = GovernanceOrigin::get();

			// Approve proposal
			assert_ok!(Treasury::approve_proposal(
				RuntimeOrigin::signed(governance_origin),
				ALICE,
				0
			));

			// Check upfront payment
			const EXPECTED_UPFRONT_PAYMENT: u128 = 400_000;
			assert_eq!(
				<Test as Config>::NativeBalance::balance(&ALICE),
				ALICE_INITIAL_BALANCE + EXPECTED_UPFRONT_PAYMENT
			);

			let initial_block_number = System::block_number();
			let mut block_number = initial_block_number;
			const EXPECTED_BALANCE_AFTER_UPFRONT: u128 =
				ALICE_INITIAL_BALANCE + EXPECTED_UPFRONT_PAYMENT;
			const AMOUNT_PER_PERIODIC_PAYMENT: u128 = 40_000;
			for i in (10..=90u128).step_by(10) {
				// Fast forward 10 blocks
				block_number = initial_block_number + i as u64;
				System::set_block_number(block_number);
				Treasury::on_initialize(block_number);

				// Check periodic payment
				let payment_instance_counter = i / 10;
				let expected_balance = EXPECTED_BALANCE_AFTER_UPFRONT
					+ AMOUNT_PER_PERIODIC_PAYMENT * (payment_instance_counter);
				assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), expected_balance);
			}

			System::set_block_number(block_number + 10);
			Treasury::on_initialize(block_number + 10);

			System::assert_last_event(
				Event::PayoutMovedForward {
					curr_block_number: 101,
					moved_to_block_number: 111,
					proposer: ALICE,
					beneficiary: ALICE,
					asset_id: 0,
					amount: AMOUNT_PER_PERIODIC_PAYMENT,
				}
				.into(),
			);

			assert_eq!(
				<Test as Config>::NativeBalance::balance(&ALICE),
				ALICE_INITIAL_BALANCE + PROPOSAL_AMOUNT
					- AFTER_FULLY_COMPLETE_AMOUNT
					- AMOUNT_PER_PERIODIC_PAYMENT
			);

			// Check Treasury balance
			assert_eq!(
				<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
				39_999
			);

			// Fund Treasury to be able to send last payment
			let fund_treasury_amount = 500_000;
			assert_ok!(Treasury::fund_treasury_native(
				RuntimeOrigin::signed(BOB),
				fund_treasury_amount
			));

			System::set_block_number(block_number + 20);
			Treasury::on_initialize(block_number + 20);

			// Assert that the last payment was sent
			assert_eq!(<Test as Config>::NativeBalance::balance(&ALICE), 900_000);

			// Confirm proposal is complete
			assert_ok!(Treasury::confirm_full_completion(
				RuntimeOrigin::signed(governance_origin),
				ALICE,
				0
			));

			// Check Alice balance
			assert_eq!(
				<Test as Config>::NativeBalance::balance(&ALICE),
				PROPOSAL_AMOUNT + ALICE_INITIAL_BALANCE
			);
		});
}

#[test]
fn exchange_funds_in_treasury() {
	StateBuilder::default().build_and_execute(|| {
		let asset_id = 1;
		let admin = 0;
		let is_sufficient = true;
		let min_balance = 1;

		// Check treasury asset 1 balance
		assert_eq!(<Test as Config>::Fungibles::balance(1, &Treasury::treasury_account_id()), 0);

		// Check treasury native balance
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
			TREASURY_INITIAL_BALANCE
		);

		assert_ok!(<<Test as Config>::Fungibles as fungibles::Create<_>>::create(
			asset_id,
			admin,
			is_sufficient,
			min_balance
		));

		// Exchange funds in treasury
		let governance_origin = GovernanceOrigin::get();
		let exchange_amount = 100_000;
		assert_ok!(Treasury::exchange_funds_in_treasury(
			RuntimeOrigin::signed(governance_origin),
			0,
			exchange_amount,
			1,
		));

		// Check Treasury native balance after exchange
		assert_eq!(
			<Test as Config>::NativeBalance::balance(&Treasury::treasury_account_id()),
			(TREASURY_INITIAL_BALANCE - exchange_amount).into()
		);
		// Check Treasury asset 1 balance after exchange
		assert_eq!(
			<Test as Config>::Fungibles::balance(1, &Treasury::treasury_account_id()),
			exchange_amount.into()
		);
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
		assert_eq!(
			<Test as Config>::Fungibles::balance(asset_id, &alice),
			<Test as pallet::Config>::AmountHeldOnProposal::get()
		);
	});
}
