#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/polkadot_sdk/frame_runtime/index.html
// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html
// https://paritytech.github.io/polkadot-sdk/master/frame_support/attr.pallet.html#dev-mode-palletdev_mode
#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use crate::AssetPriceLookup;
	use frame_support::sp_runtime::traits::AccountIdConversion;
	use frame_support::traits::fungible::MutateHold;
	use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
	use frame_support::PalletId;
	use frame_support::{
		pallet_prelude::*,
		traits::{
			fungible::{self, Mutate as FungibleMutate},
			fungibles::{self, Mutate as FungiblesMutate},
			EnsureOrigin,
		},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use sp_runtime::Percent;

	const PALLET_ID: PalletId = PalletId(*b"treasury");

	pub type AssetIdOf<T> = <<T as Config>::Fungibles as fungibles::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::AssetId;

	pub type BalanceOf<T> = <<T as Config>::NativeBalance as fungible::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	pub type AssetBalanceOf<T> = <<T as Config>::Fungibles as fungibles::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/reference_docs/frame_runtime_types/index.html
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Type to access the Balances Pallet.
		type NativeBalance: fungible::Inspect<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::hold::Inspect<Self::AccountId>
			+ fungible::hold::Mutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId>;

		/// Type to access the Assets Pallet.
		type Fungibles: fungibles::Inspect<Self::AccountId, Balance = BalanceOf<Self>>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::Create<Self::AccountId>;

		type RuntimeHoldReason: From<HoldReason>;

		const NATIVE_ASSET_ID: AssetIdOf<Self>;

		// two ways to convert asset and balance type to one another, look into `ConvertBack` for
		// reverse conversion, or define a second type.
		// type AssetIdToBalance: Convert<AssetBalanceOf<Self>, BalanceOf<Self>>;
		// fn asset_id_to_balance(id: AssetBalanceOf<Self>) -> BalanceOf<Self>;
		// or, do something like this:
		// type Fungibles: fungibles::Inspect<Self::AccountId, Balance = BalanceOf<Self>>

		// Governance origin to ensure the origin of a call is authorized
		// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/reference_docs/frame_origin/index.html#asserting-on-a-custom-external-origin
		type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		// Here is an associated type to give you access to your simple asset price lookup function
		type AssetPriceLookup: crate::AssetPriceLookup<Self>;

		// Thresholds for different spender categories
		#[pallet::constant]
		type SmallSpenderThreshold: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type MediumSpenderThreshold: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type AmountHeldOnProposal: Get<BalanceOf<Self>>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#event-and-error
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// index_count is the number of proposals the proposer has made, starting from 0
		AddedProposal {
			proposer: T::AccountId,
			index_count: u16,
			amount: BalanceOf<T>,
			title: BoundedVec<u8, ConstU32<32>>,
		},
		PayoutMovedForward {
			curr_block_number: BlockNumberFor<T>,
			moved_to_block_number: BlockNumberFor<T>,
			proposer: T::AccountId,
			beneficiary: T::AccountId,
			asset_id: AssetIdOf<T>,
			amount: BalanceOf<T>,
		},
	}

	/// Errors inform users that something went wrong.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#event-and-error
	#[pallet::error]
	pub enum Error<T> {
		PayoutPercentagesMustSumTo100,
		PaymentEachNBlocksMustBeGreaterThanZero,
	}

	#[pallet::origin]
	#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum Origin {
		SmallSpender,
		MediumSpender,
		BigSpender,
	}

	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen, Debug, Clone, PartialEq)]
	pub enum NumOfPeriodicPayouts {
		Five = 5,
		Ten = 10,
		Twenty = 20,
		Fifty = 50,
	}

	// full amount = upfront + periodic + after_fully_complete
	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen, Debug, Clone, PartialEq)]
	pub struct PeriodicPayoutPercentage {
		pub upfront: u8,
		pub periodic: u8,
		pub after_fully_complete: u8,

		pub num_of_periodic_payouts: NumOfPeriodicPayouts,
		pub payment_each_n_blocks: u32,
	}

	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen, Debug, Clone, PartialEq)]
	pub enum PayoutType {
		Periodic(PeriodicPayoutPercentage),
		Instant,
	}

	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen, Debug, Clone, PartialEq)]
	#[scale_info(skip_type_params(T))]
	pub struct PeriodicPayoutInstance<T: Config> {
		proposer: T::AccountId,
		// proposal_index: u16,
		beneficiary: T::AccountId,
		asset_id: AssetIdOf<T>,
		amount: BalanceOf<T>,
	}

	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen, Debug)]
	#[scale_info(skip_type_params(T))]
	pub struct SpendingProposal<T: Config> {
		title: BoundedVec<u8, ConstU32<32>>,
		description: BoundedVec<u8, ConstU32<500>>,
		proposer: T::AccountId,
		beneficiary: T::AccountId,
		#[codec(compact)]
		amount: BalanceOf<T>,
		asset_id: AssetIdOf<T>,
		spender_type: Origin,
		payout_type: PayoutType,
		pub approved: bool,
		pub completed: bool,
	}

	/// The pallet's storage items.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#storage
	/// https://paritytech.github.io/polkadot-sdk/master/frame_support/pallet_macros/attr.storage.html
	#[pallet::storage]
	pub type SpendingProposals<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u16, SpendingProposal<T>>;

	#[pallet::storage]
	pub type NumOfProposalsFromProposer<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, u16, ValueQuery, GetDefault>;

	#[pallet::storage]
	pub type PayoutInstances<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, Vec<PeriodicPayoutInstance<T>>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig;

	#[pallet::genesis_build]
	impl BuildGenesisConfig for GenesisConfig {
		fn build(&self) {}
	}

	/// A reason for placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds held for stake proposing spend.
		#[codec(index = 0)]
		SpendingProposal,
	}

	/// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	/// These functions materialize as "extrinsics", which are often compared to transactions.
	/// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#dispatchables
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		pub fn fund_treasury_asset(
			origin: OriginFor<T>,
			amount: BalanceOf<T>,
			asset_id: AssetIdOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			T::Fungibles::transfer(
				asset_id,
				&who,
				&Self::treasury_account_id(),
				amount,
				Preservation::Expendable,
			)?;
			Ok(())
		}

		pub fn fund_treasury_native(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			T::NativeBalance::transfer(
				&who,
				&Self::treasury_account_id(),
				amount,
				Preservation::Expendable,
			)?;
			Ok(())
		}

		pub fn propose_spend(
			origin: OriginFor<T>,
			title: BoundedVec<u8, ConstU32<32>>,
			description: BoundedVec<u8, ConstU32<500>>,
			asset_id: AssetIdOf<T>,
			amount: BalanceOf<T>,
			proposer: T::AccountId,
			beneficiary: T::AccountId,
			payout_type: PayoutType,
		) -> DispatchResult {
			let _anyone = ensure_signed(origin)?;
			Self::do_propose_spend(
				title,
				description,
				asset_id,
				amount,
				proposer,
				beneficiary,
				payout_type,
			)
		}

		pub fn approve_proposal(
			origin: OriginFor<T>,
			proposer: T::AccountId,
			index: u16,
		) -> DispatchResult {
			let _who = T::GovernanceOrigin::ensure_origin(origin)?;
			SpendingProposals::<T>::try_mutate(&proposer, index, |proposal| match proposal {
				Some(p) => {
					if p.approved {
						return Err("Proposal already approved");
					}
					p.approved = true;
					Self::setup_payout_instances(p)?;
					return Ok(());
				},
				None => return Err("Proposal does not exist"),
			})?;

			T::NativeBalance::release(
				&HoldReason::SpendingProposal.into(),
				&proposer,
				T::AmountHeldOnProposal::get(),
				Precision::BestEffort,
			)?;
			Ok(())
		}

		pub fn exchange_funds_in_treasury(
			origin: OriginFor<T>,
			asset_id_a: AssetIdOf<T>,
			amount_a: BalanceOf<T>,
			asset_id_b: AssetIdOf<T>,
		) -> DispatchResult {
			let _who = T::GovernanceOrigin::ensure_origin(origin)?;
			let amount_b =
				T::AssetPriceLookup::price_lookup(asset_id_a.clone(), amount_a, asset_id_b.clone());

			if asset_id_a == T::NATIVE_ASSET_ID {
				T::NativeBalance::burn_from(
					&Self::treasury_account_id(),
					amount_a,
					Preservation::Preserve,
					Precision::BestEffort,
					Fortitude::Polite,
				)?;
			} else {
				T::Fungibles::burn_from(
					asset_id_a,
					&Self::treasury_account_id(),
					amount_a,
					Preservation::Preserve,
					Precision::BestEffort,
					Fortitude::Polite,
				)?;
			}

			if asset_id_b == T::NATIVE_ASSET_ID {
				T::NativeBalance::mint_into(&Self::treasury_account_id(), amount_b)?;
			} else {
				T::Fungibles::mint_into(asset_id_b, &Self::treasury_account_id(), amount_b)?;
			}
			Ok(())
		}

		pub fn confirm_full_completion(
			origin: OriginFor<T>,
			proposer: T::AccountId,
			index: u16,
		) -> DispatchResult {
			let _who = T::GovernanceOrigin::ensure_origin(origin)?;
			SpendingProposals::<T>::try_mutate(&proposer, index, |proposal| match proposal {
				Some(p) => {
					if !p.approved {
						return Err("Proposal not approved");
					}
					if p.completed {
						return Err("Proposal already completed");
					}

					p.completed = true;

					if let PayoutType::Periodic(periodic_payout_percentage) = &p.payout_type {
						if periodic_payout_percentage.after_fully_complete > 0 {
							if p.asset_id == T::NATIVE_ASSET_ID {
								Self::send_native_funds_to_beneficiary(
									&p.beneficiary,
									Percent::from_percent(
										periodic_payout_percentage.after_fully_complete,
									) * p.amount,
								)?;
							} else {
								Self::send_asset_funds_to_beneficiary(
									&p.beneficiary,
									Percent::from_percent(
										periodic_payout_percentage.after_fully_complete,
									) * p.amount,
									&p.asset_id,
								)?;
							}
						}
					}

					return Ok(());
				},
				None => return Err("Proposal does not exist"),
			})?;
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Proverava da li ima schedulovanih isplata za trenutni blok i isplati ih
		/// Ako nema dovoljno sredstava, isplata se pomera za 10 blokova unapred
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			let payout_instances = PayoutInstances::<T>::get(_n);
			for payout in &payout_instances {
				let res = Self::exec_payout_instance(&payout);
				if res.is_err() {
					let curr_block_number = <frame_system::Pallet<T>>::block_number();
					let moved_to_block_number = curr_block_number + 10u32.into();
					PayoutInstances::append(moved_to_block_number, payout);
					Self::deposit_event(Event::PayoutMovedForward {
						curr_block_number,
						moved_to_block_number,
						proposer: payout.proposer.clone(),
						beneficiary: payout.beneficiary.clone(),
						asset_id: payout.asset_id.clone(),
						amount: payout.amount,
					});
				}
			}

			// Ovde treba neki weight
			Weight::zero()
		}
	}

	// NOTE: This is regular rust, and how you would implement functions onto an object.
	// These functions are NOT extrinsics, are NOT callable by outside users, and are really
	// just internal functions.
	//
	// Compare this to the block above which has `#[pallet::call]` which makes them extrinsics!
	impl<T: Config> Pallet<T> {
		pub fn treasury_account_id() -> T::AccountId {
			PALLET_ID.into_account_truncating()
		}

		pub fn setup_payout_instances(proposal: &SpendingProposal<T>) -> DispatchResult {
			match &proposal.payout_type {
				PayoutType::Periodic(payout) => {
					let upfront_amount = Percent::from_percent(payout.upfront) * proposal.amount;
					// TODO: Implement after_fully_complete
					// let after_fully_complete_amount =
					// Percent::from_percent(payout.after_fully_complete) * proposal.amount;
					let periodic_amount = Percent::from_percent(payout.periodic) * proposal.amount;

					let number_of_payout_instances = payout.num_of_periodic_payouts.clone() as u8;
					let payment_each_n_blocks = payout.payment_each_n_blocks;
					let payout_instance_amount: BalanceOf<T> =
						Percent::from_percent(100 / number_of_payout_instances) * periodic_amount;

					// Send upfront amount to beneficiary
					if proposal.asset_id == T::NATIVE_ASSET_ID {
						Self::send_native_funds_to_beneficiary(
							&proposal.beneficiary,
							upfront_amount,
						)?;
					} else {
						Self::send_asset_funds_to_beneficiary(
							&proposal.beneficiary,
							upfront_amount,
							&proposal.asset_id,
						)?;
					}

					// Setup periodic payouts
					let curr_block_number: BlockNumberFor<T> =
						<frame_system::Pallet<T>>::block_number();
					for i in 1..=number_of_payout_instances {
						let block_number: BlockNumberFor<T> =
							curr_block_number + (i as u32 * payment_each_n_blocks).into();
						let payout_instance: PeriodicPayoutInstance<T> = PeriodicPayoutInstance {
							proposer: proposal.proposer.clone(),
							// proposal_index: proposal.,
							beneficiary: proposal.beneficiary.clone(),
							asset_id: proposal.asset_id.clone(),
							amount: payout_instance_amount,
						};

						PayoutInstances::append(block_number, payout_instance);
					}
				},
				PayoutType::Instant => {
					if proposal.asset_id == T::NATIVE_ASSET_ID {
						Self::send_native_funds_to_beneficiary(
							&proposal.beneficiary,
							proposal.amount,
						)?;
					} else {
						Self::send_asset_funds_to_beneficiary(
							&proposal.beneficiary,
							proposal.amount,
							&proposal.asset_id,
						)?;
					}
				},
			}

			Ok(())
		}

		pub fn send_native_funds_to_beneficiary(
			beneficiary: &T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			T::NativeBalance::transfer(
				&Self::treasury_account_id(),
				&beneficiary,
				amount,
				Preservation::Expendable,
			)?;
			Ok(())
		}

		pub fn send_asset_funds_to_beneficiary(
			beneficiary: &T::AccountId,
			amount: BalanceOf<T>,
			asset_id: &AssetIdOf<T>,
		) -> DispatchResult {
			T::Fungibles::transfer(
				asset_id.clone(),
				&Self::treasury_account_id(),
				&beneficiary,
				amount,
				Preservation::Expendable,
			)?;
			Ok(())
		}

		fn check_payout_type(payout_type: &PayoutType) -> DispatchResult {
			match payout_type {
				PayoutType::Periodic(payout) => {
					ensure!(
						payout.upfront + payout.after_fully_complete + payout.periodic == 100,
						Error::<T>::PayoutPercentagesMustSumTo100
					);
					ensure!(
						payout.payment_each_n_blocks > 0,
						Error::<T>::PaymentEachNBlocksMustBeGreaterThanZero
					);
				},
				PayoutType::Instant => {},
			}
			Ok(())
		}

		fn exec_payout_instance(payout_instance: &PeriodicPayoutInstance<T>) -> DispatchResult {
			if payout_instance.asset_id == T::NATIVE_ASSET_ID {
				Self::send_native_funds_to_beneficiary(
					&payout_instance.beneficiary,
					payout_instance.amount,
				)?;
			} else {
				Self::send_asset_funds_to_beneficiary(
					&payout_instance.beneficiary,
					payout_instance.amount,
					&payout_instance.asset_id,
				)?;
			}
			Ok(())
		}

		fn do_propose_spend(
			title: BoundedVec<u8, ConstU32<32>>,
			description: BoundedVec<u8, ConstU32<500>>,
			asset_id: AssetIdOf<T>,
			amount: BalanceOf<T>,
			proposer: T::AccountId,
			beneficiary: T::AccountId,
			payout_type: PayoutType,
		) -> DispatchResult {
			Self::check_payout_type(&payout_type)?;

			T::NativeBalance::hold(&HoldReason::SpendingProposal.into(), &proposer, 100u32.into())?;

			let price_in_usd = T::AssetPriceLookup::usd_price(&asset_id, amount);

			// Determine the spender type based on the amount
			let spender_type = if price_in_usd <= T::SmallSpenderThreshold::get() {
				Origin::SmallSpender
			} else if amount <= T::MediumSpenderThreshold::get() {
				Origin::MediumSpender
			} else {
				Origin::BigSpender
			};

			let index_count = NumOfProposalsFromProposer::<T>::get(&proposer);

			let proposal = SpendingProposal {
				title: title.clone(),
				description,
				proposer: proposer.clone(),
				beneficiary,
				amount,
				asset_id,
				spender_type,
				approved: false,
				payout_type,
				completed: false,
			};

			SpendingProposals::<T>::insert(&proposer, index_count, proposal);
			NumOfProposalsFromProposer::<T>::insert(&proposer, index_count + 1);

			Self::deposit_event(Event::AddedProposal {
				proposer,
				index_count,
				amount,
				title: title.clone(),
			});

			Ok(())
		}
	}
}

/// This is some simple function that can be used to convert some amount of Asset A, and turn it
/// into some amount of Asset B. You do not really need to implement this function. You would to
/// build your own complex pallet to figure this out (oracle, dex) BUT you can make the assumption
/// that somewhere this logic exists, and then you can use it.
pub trait AssetPriceLookup<T: Config> {
	fn price_lookup(
		asset_a_id: AssetIdOf<T>,
		asset_a_amount: AssetBalanceOf<T>,
		asset_b_id: AssetIdOf<T>,
	) -> AssetBalanceOf<T>;

	fn usd_price(asset_id: &AssetIdOf<T>, amount: AssetBalanceOf<T>) -> AssetBalanceOf<T>;
}
