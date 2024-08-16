//! Benchmarking setup for pallet-treasury
#![cfg(feature = "runtime-benchmarks")]
use super::*;

#[allow(unused)]
use crate::Pallet as Treasury;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::fungible::Inspect;
use frame_support::traits::fungibles::Inspect as FungiblesInspect;
use frame_support::traits::fungibles::Mutate as FungiblesMutate;
use frame_support::traits::fungibles::Create;

/// Benchmark Helper
pub trait BenchmarkHelper<AssetId> {
    fn convert_to_asset_id(id: u32) -> AssetId;
}

impl<AssetId> BenchmarkHelper<AssetId> for ()
where
    AssetId: From<u32>,
{
    fn convert_to_asset_id(id: u32) -> AssetId {
        id.into()
    }
}

#[benchmarks]
mod benchmarks {
	use super::*;

	// #[benchmark]
	// fn do_something() {
	// 	let value = 100u32.into();
	// 	let caller: T::AccountId = whitelisted_caller();
	// 	#[extrinsic_call]
	// 	do_something(RawOrigin::Signed(caller), value);

	// 	assert_eq!(Something::<T>::get(), Some(value));
	// }

	// #[benchmark]
	// fn cause_error() {
	// 	Something::<T>::put(100u32);
	// 	let caller: T::AccountId = whitelisted_caller();
	// 	#[extrinsic_call]
	// 	cause_error(RawOrigin::Signed(caller));

	// 	assert_eq!(Something::<T>::get(), Some(101u32));
	// }

	#[benchmark]
	fn fund_treasury_asset() {
		let caller: T::AccountId = whitelisted_caller();
		let value = 1_000_000_u32.into();
		let asset_id = T::BenchmarkHelper::convert_to_asset_id(1);

		T::Fungibles::create(
			asset_id.clone(),
			caller.clone(),
			true,
			1_u32.into()
		);	

		T::Fungibles::set_balance(asset_id.clone(), &caller, value);
		// let caller_balance = T::NativeBalance::get().free_balance(&caller);
		let treasury_balance = T::Fungibles::balance(asset_id.clone(), &Pallet::<T>::treasury_account_id());

		#[extrinsic_call]
		Treasury::fund_treasury_asset(
			RawOrigin::Signed(caller.clone()),
			100_000_u32.into(),
			asset_id.clone()
		);

		assert_eq!(
			T::Fungibles::balance(asset_id.clone(), &caller),
			900_000_u32.into()
		);
		assert_eq!(
			T::Fungibles::balance(asset_id, &Pallet::<T>::treasury_account_id()),
			100_000_u32.into()
		);
	}

	#[benchmark]
	fn fund_treasury_native() {
		let caller: T::AccountId = whitelisted_caller();
		let value = 1_000_000_u32.into();
		T::NativeBalance::set_balance(&caller, value);
		// let caller_balance = T::NativeBalance::get().free_balance(&caller);
		let treasury_balance = T::NativeBalance::balance(&Pallet::<T>::treasury_account_id());

		#[extrinsic_call]
		Treasury::fund_treasury_native(
			RawOrigin::Signed(caller.clone()),
			100_000_u32.into()
		);

		assert_eq!(
			T::NativeBalance::balance(&caller),
			900_000_u32.into()
		);
		assert_eq!(
			T::NativeBalance::balance(&Pallet::<T>::treasury_account_id()),
			100_000_u32.into()
		);
	}


	impl_benchmark_test_suite!(Treasury, crate::mock::new_test_ext(), crate::mock::Test);
}
