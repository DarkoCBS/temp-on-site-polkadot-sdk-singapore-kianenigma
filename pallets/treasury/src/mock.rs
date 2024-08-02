use crate as pallet_treasury;
use frame_support::ord_parameter_types;
use frame_support::{
	derive_impl,
	traits::{AsEnsureOriginWithArg, ConstU128, ConstU16, ConstU32, ConstU64},
};
use frame_support::{parameter_types, PalletId};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use sp_core::H256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
pub type Balance = u128;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub struct Test {
		System: frame_system,
		Balances: pallet_balances,
		Assets: pallet_assets,
		Treasury: pallet_treasury,
	}
);

// Feel free to remove more items from this, as they are the same as
// `frame_system::config_preludes::TestDefaultConfig`. We have only listed the full `type` list here
// for verbosity. Same for `pallet_balances::Config`.
// https://paritytech.github.io/polkadot-sdk/master/frame_support/attr.derive_impl.html
#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ConstU32<10>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ConstU32<10>;
}

impl pallet_assets::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type AssetDeposit = ConstU128<100>;
	type AssetAccountDeposit = ConstU128<1>;
	type MetadataDepositBase = ConstU128<10>;
	type MetadataDepositPerByte = ConstU128<1>;
	type ApprovalDeposit = ConstU128<1>;
	type StringLimit = ConstU32<50>;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = ();
	type RemoveItemsLimit = ConstU32<1000>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

// In a real blockchain, this wouldn't be some simple function, but actually a whole pallet that
// would implement this logic. But for your situation, and unit tests, you just need a simple
// implementation.
pub struct SimplePriceLookup;
impl crate::AssetPriceLookup<Test> for SimplePriceLookup {
	fn price_lookup(_id_a: u32, amt_a: u128, _id_b: u32) -> u128 {
		// Here we make the simple assumption asset a is always the same amount as asset b
		// you can write more clever function here for more accurate testing
		amt_a
	}

	fn usd_price(
		asset_id: &crate::AssetIdOf<Test>,
		amount: crate::AssetBalanceOf<Test>,
	) -> crate::AssetBalanceOf<Test> {
		amount
	}
}

parameter_types! {
	pub static SmallSpenderThreshold: u32 = 5000;
	pub static MediumSpenderThreshold: u32 = 20000;
	pub static GovernancePalletId: PalletId = PalletId(*b"test/gov");
}

ord_parameter_types! {
	pub const GovernanceOrigin: u64 = AccountIdConversion::<u64>::into_account_truncating(&GovernancePalletId::get());
}

impl pallet_treasury::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type NativeBalance = Balances;
	type Fungibles = Assets;
	type GovernanceOrigin = EnsureSignedBy<GovernanceOrigin, u64>;
	type AssetPriceLookup = SimplePriceLookup;
	const NATIVE_ASSET_ID: crate::AssetIdOf<Self> = 0;
	type SmallSpenderThreshold = SmallSpenderThreshold;
	type MediumSpenderThreshold = MediumSpenderThreshold;
	type RuntimeHoldReason = RuntimeHoldReason;
}

// pub fn new_test_ext() -> sp_io::TestExternalities {
// 	// learn how to improve your test setup:
// 	// https://paritytech.github.io/polkadot-sdk/master/polkadot_sdk_docs/guides/your_first_pallet/index.html
// 	// frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
// 	RuntimeGenesisConfig::default().build_storage().unwrap().into()
// }
