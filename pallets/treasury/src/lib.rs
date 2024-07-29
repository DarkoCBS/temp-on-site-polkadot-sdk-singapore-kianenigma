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
	use frame_support::traits::tokens::{AssetId, Preservation};
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

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum Origin {
		SmallSpender,
		MediumSpender,
		BigSpender,
	}

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
			+ fungible::hold::Mutate<Self::AccountId>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId>;

		/// Type to access the Assets Pallet.
		type Fungibles: fungibles::Inspect<Self::AccountId, Balance = BalanceOf<Self>>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::Create<Self::AccountId>;

		const NATIVE_ASSET_ID: AssetIdOf<Self>;

		// two ways to convert asset and balance type to one another, look into `ConvertBack` for
		// reverse conversion, or define a second type.
		// type AssetIdToBalance: Convert<AssetBalanceOf<Self>, BalanceOf<Self>>;
		// fn asset_id_to_balance(id: AssetBalanceOf<Self>) -> BalanceOf<Self>;
		// or, do something like this:
		// type Fungibles: fungibles::Inspect<Self::AccountId, Balance = BalanceOf<Self>>

		// A custom, configurable origin that you can use. It can be wired to be `EnsureSigned`,
		// `EnsureRoot`, or any custom implementation at the runtime level.
		// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/reference_docs/frame_origin/index.html#asserting-on-a-custom-external-origin
		type TreasuryOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		// Here is an associated type to give you access to your simple asset price lookup function
		type AssetPriceLookup: crate::AssetPriceLookup<Self>;

		// type SmallSpender: EnsureOrigin<Self::RuntimeOrigin>;
		// type BigSpender: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type SmallSpenderThreshold: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type MediumSpenderThreshold: Get<BalanceOf<Self>>;
	}

	/// The pallet's storage items.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#storage
	/// https://paritytech.github.io/polkadot-sdk/master/frame_support/pallet_macros/attr.storage.html

	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct SpendingProposal<T: Config> {
		title: [u8; 32],
		description: [u8; 500],
		proposer: T::AccountId,
		beneficiary: T::AccountId,
		#[codec(compact)]
		amount: BalanceOf<T>,
		asset_id: AssetIdOf<T>,
		spender_type: Origin,
		approved: bool,
	}

	#[pallet::storage]
	pub type SpendingProposals<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u16, SpendingProposal<T>>;

	#[pallet::storage]
	pub type NumOfProposalsFromProposer<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, u16, ValueQuery, GetDefault>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig;

	#[pallet::genesis_build]
	impl BuildGenesisConfig for GenesisConfig {
		fn build(&self) {
			const PALLET_ID: PalletId = PalletId(*b"treasury");
			PALLET_ID.try_into_account().expect("Failed to create account ID")
		}
	}

	/// Errors inform users that something went wrong.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#event-and-error
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// We usually use passive tense for events.
		SomethingStored { something: u32, who: T::AccountId },
		// index_count is the number of proposals the proposer has made, starting from 0
		AddedProposal {
			proposer: T::AccountId,
			index_count: u16,
			amount: BalanceOf<T>,
			title: [u8; 32],
		},
	}

	/// Errors inform users that something went wrong.
	/// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html#event-and-error
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

	// Here is an example of explicitly telling SCALE codec to encode a number as compact in
	// storage.
	#[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct StoreACompactNumber<T: Config> {
		who: T::AccountId,
		#[codec(compact)]
		amount: BalanceOf<T>,
	}

	#[pallet::storage]
	pub type MyCompactNumber<T: Config> = StorageValue<Value = StoreACompactNumber<T>>;

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

		pub fn approve_proposal(
			origin: OriginFor<T>,
			proposer: T::AccountId,
			index: u16,
		) -> DispatchResult {
			let _who = T::TreasuryOrigin::ensure_origin(origin)?;
			SpendingProposals::<T>::try_mutate(&proposer, index, |proposal| {
				match proposal {
					Some(p) => {
						if p.approved {
							return Err("Proposal already approved");
						}
						p.approved = true;
						if p.asset_id == T::NATIVE_ASSET_ID {
							Self::send_native_funds_to_beneficiary(&p.beneficiary, p.amount)?;
						} else {
							Self::send_asset_funds_to_beneficiary(&p.beneficiary, p.amount, p.asset_id.clone())?;
						}
						Ok(())
					},
					None => Err("Proposal does not exist"),
				}
			})?;
		
			Ok(())
		}

		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		pub fn do_something(origin: OriginFor<T>, something: u32) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// // https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/reference_docs/frame_origin/index.html
			let who = ensure_signed(origin)?;

			// Update storage.
			// <Something<T>>::put(something);

			// Emit an event.
			Self::deposit_event(Event::SomethingStored { something, who });

			Ok(())
		}

		// /// An example dispatchable that may throw a custom error.
		// pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
		// 	let _who = ensure_signed(origin)?;

		// 	Ok(())
		// 	// Read a value from storage.
		// 	match <Something<T>>::get() {
		// 		// Return an error if the value has not been set.
		// 		None => Err(Error::<T>::NoneValue.into()),
		// 		Some(old) => {
		// 			// Increment the value read from storage; will error in the event of overflow.
		// 			let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
		// 			// Update the value in storage with the incremented result.
		// 			<Something<T>>::put(new);
		// 			Ok(())
		// 		},
		// 	}
		// }

		pub fn propose_spend(
			origin: OriginFor<T>,
			title: [u8; 32],
			description: [u8; 500],
			asset_id: AssetIdOf<T>,
			amount: BalanceOf<T>,
			proposer: T::AccountId,
			beneficiary: T::AccountId,
		) -> DispatchResult {
			let _anyone = ensure_signed(origin)?;
			Self::do_propose_spend(title, description, asset_id, amount, proposer, beneficiary)
		}

		// Let's imagine you wanted to build a transfer extrinsic inside your pallet...
		// This doesn't really make sense to do, since this functionality exists in the `Balances`
		// pallet, but it illustrates how to use the `BalanceOf<T>` type and the `T::NativeBalance`
		// api.
		pub fn my_transfer_function(
			origin: OriginFor<T>,
			to: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			// Probably you would import these at the top of you file, not here, but just trying to
			// illustrate that you need to import this trait to access the function inside of it.
			use frame_support::traits::fungible::Mutate;
			// Read the docs on this to understand what it does...
			use frame_support::traits::tokens::Preservation;

			let sender = ensure_signed(origin)?;
			T::NativeBalance::transfer(&sender, &to, amount, Preservation::Expendable)?;
			Ok(())
		}
	}

	// NOTE: This is regular rust, and how you would implement functions onto an object.
	// These functions are NOT extrinsics, are NOT callable by outside users, and are really
	// just internal functions.
	//
	// Compare this to the block above which has `#[pallet::call]` which makes them extrinsics!
	impl<T: Config> Pallet<T> {
		pub fn treasury_account_id() -> T::AccountId {
			PALLET_ID.try_into_account().expect("Failed to create account ID")
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
			asset_id: AssetIdOf<T>,
		) -> DispatchResult {
			T::Fungibles::transfer(
				asset_id,
				&Self::treasury_account_id(),
				&beneficiary,
				amount,
				Preservation::Expendable,
			)?;
			Ok(())
		}

		fn do_propose_spend(
			title: [u8; 32],
			description: [u8; 500],
			asset_id: AssetIdOf<T>,
			amount: BalanceOf<T>,
			proposer: T::AccountId,
			beneficiary: T::AccountId,
		) -> DispatchResult {
			// Write the logic for your extrinsic here, since this is "outside" of the macros.
			// Following this kind of best practice can even allow you to move most of your
			// pallet logic into different files, with better, more clear structure, rather
			// than having a single huge complicated file.

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
				title,
				description,
				proposer: proposer.clone(),
				beneficiary,
				amount,
				asset_id,
				spender_type,
				approved: false,
			};

			SpendingProposals::<T>::insert(&proposer, index_count, proposal);
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
