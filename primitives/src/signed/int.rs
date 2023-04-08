use core::fmt;
use ruint::Uint;

#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

use super::{
    errors::{self},
    utils::*,
    Sign,
};

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
/// Signed integer wrapping a `ruint::Uint`.
///
/// This signed integer implementation is fully abstract across the number of
/// bits. It wraps a [`ruint::Uint`], and co-opts the most significant bit to
/// represent the sign. The number is represented in two's complement, using the
/// underlying `Uint`'s `u64` limbs. The limbs can be accessed via the
/// [`Signed::as_limbs()`] method, and are least-significant first.
///
/// ## Aliases
///
/// We provide aliases for every bit-width divisble by 8, from 8 to 256. These
/// are located in [`crate::aliases`] and are named `I256`, `I248` etc. Most
/// users will want [`crate::I256`].
///
/// # Usage
///
/// ```
/// # use ethers_primitives::I256;
/// // Instantiate from a number
/// let a = I256::unchecked_from(1);
/// // Use `try_from` if you're not sure it'll fit
/// let b = I256::try_from(200000382).unwrap();
///
/// // Or parse from a string :)
/// let c = "100".parse::<I256>().unwrap();
/// let d = "-0x138f".parse::<I256>().unwrap();
///
/// // Preceding plus is allowed but not recommended
/// let e = "+0xdeadbeef".parse::<I256>().unwrap();
///
/// // Underscores are ignored
/// let f = "1_000_000".parse::<I256>().unwrap();
///
/// // But invalid chars are not
/// assert!("^31".parse::<I256>().is_err());
///
/// // Omitting the hex prefix is allowed, but not recommended
/// // Be careful, it can be confused for a decimal string!
/// let g = "deadbeef".parse::<I256>().unwrap();
/// // Is this hex? or decimal?
/// let h = "1113".parse::<I256>().unwrap();
/// // It's decimal!
/// assert_eq!(h, I256::unchecked_from(1113));
///
/// // Math works great :)
/// let g = a * b + c - d;
///
/// // And so does comparison!
/// assert!(e > a);
///
/// // We have some useful constants too
/// assert_eq!(I256::zero(), I256::unchecked_from(0));
/// assert_eq!(I256::one(), I256::unchecked_from(1));
/// assert_eq!(I256::minus_one(), I256::unchecked_from(-1));
/// ```
///
/// # Note on [`std::str::FromStr`]
///
/// The parse function first tries the string as a decimal string, then as a
/// hex string. We do it this way because decimal has a more-restrictive
/// alphabet. E.g. the string "11f" is valid hex but not valid decimal. This
/// means that errors are reported more correctly (there are no false invalid
/// char errors on valid-but-overflowing hex strings). However, this means that
/// when using un-prefixed hex strings, they will be confused for decimal
/// strings if they use no hex digits.
///
/// To prevent this, we strongly recommend always prefixing hex strings with
/// `0x` AFTER the sign (if any).
pub struct Signed<const BITS: usize, const LIMBS: usize>(pub(crate) Uint<BITS, LIMBS>);

// formatting
impl<const BITS: usize, const LIMBS: usize> fmt::Debug for Signed<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<const BITS: usize, const LIMBS: usize> fmt::Display for Signed<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (sign, abs) = self.into_sign_and_abs();
        fmt::Display::fmt(&sign, f)?;
        write!(f, "{abs}")
    }
}

impl<const BITS: usize, const LIMBS: usize> fmt::LowerHex for Signed<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (sign, abs) = self.into_sign_and_abs();
        fmt::Display::fmt(&sign, f)?;
        write!(f, "{abs:x}")
    }
}

impl<const BITS: usize, const LIMBS: usize> fmt::UpperHex for Signed<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (sign, abs) = self.into_sign_and_abs();
        fmt::Display::fmt(&sign, f)?;

        // NOTE: Work around `U256: !UpperHex`.
        let mut buffer = format!("{abs:x}");
        buffer.make_ascii_uppercase();
        f.write_str(&buffer)
    }
}

impl<const BITS: usize, const LIMBS: usize> Signed<BITS, LIMBS> {
    /// Mask for the highest limb
    pub(crate) const MASK: u64 = mask(BITS);

    /// Location of the sign bit within the highest limb
    pub(crate) const SIGN_BIT: u64 = sign_bit(BITS);

    /// Number of bits
    pub const BITS: usize = BITS;

    /// The minimum value
    pub const MIN: Self = min();

    /// The maximum value
    pub const MAX: Self = max();

    /// Zero (additive identity) of this type.
    pub const ZERO: Self = zero();

    /// One (multiplicative identity) of this type.
    pub const ONE: Self = one();

    /// Minus one (multiplicative inverse) of this type.
    pub const MINUS_ONE: Self = Self(Uint::<BITS, LIMBS>::MAX);

    /// Zero (additive iden
    #[inline(always)]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// One (multiplicative identity) of this type.
    #[inline(always)]
    pub const fn one() -> Self {
        Self::ONE
    }

    /// Minus one (multiplicative inverse) of this type.
    #[inline(always)]
    pub const fn minus_one() -> Self {
        Self::MINUS_ONE
    }

    /// The maximum value which can be inhabited by this type.
    #[inline(always)]
    pub const fn max_value() -> Self {
        Self::MAX
    }

    /// The minimum value which can be inhabited by this type.
    #[inline(always)]
    pub const fn min_value() -> Self {
        Self::MIN
    }

    /// Coerces an unsigned integer into a signed one. If the unsigned integer
    /// is greater than the greater than or equal to `1 << 255`, then the result
    /// will overflow into a negative value.
    #[inline(always)]
    pub const fn from_raw(val: Uint<BITS, LIMBS>) -> Self {
        Self(val)
    }

    /// Attempt to perform the conversion via a `TryInto` implementation, and
    /// panic on failure
    ///
    /// This is a shortcut for `val.try_into().unwrap()`
    #[inline(always)]
    pub fn unchecked_from<T>(val: T) -> Self
    where
        T: TryInto<Self>,
        <T as TryInto<Self>>::Error: fmt::Debug,
    {
        val.try_into().unwrap()
    }

    /// Attempt to perform the conversion via a `TryInto` implementation, and
    /// panic on failure
    ///
    /// This is a shortcut for `self.try_into().unwrap()`
    #[inline(always)]
    pub fn unchecked_into<T>(self) -> T
    where
        Self: TryInto<T>,
        <Self as TryInto<T>>::Error: fmt::Debug,
    {
        self.try_into().unwrap()
    }

    /// Returns the signed integer as a unsigned integer. If the value of `self` negative, then the
    /// two's complement of its absolute value will be returned.
    #[inline(always)]
    pub const fn into_raw(self) -> Uint<BITS, LIMBS> {
        self.0
    }

    /// Returns the sign of self.
    #[inline(always)]
    pub const fn sign(self) -> Sign {
        // if the last limb contains the sign bit, then we're negative
        // because we can't set any higher bits to 1, we use >= as a proxy
        // check to avoid bit comparison
        if let Some(limb) = self.0.as_limbs().last() {
            if *limb >= Self::SIGN_BIT {
                return Sign::Negative;
            }
        }
        Sign::Positive
    }

    /// Determines if the integer is odd
    pub const fn is_odd(self) -> bool {
        if BITS == 0 {
            false
        } else {
            self.as_limbs()[0] % 2 == 1
        }
    }

    /// Returns `true` if `self` is zero and `false` if the number is negative
    /// or positive.
    #[inline(always)]
    pub const fn is_zero(self) -> bool {
        const_eq(self, Self::ZERO)
    }

    /// Returns `true` if `self` is positive and `false` if the number is zero
    /// or negative
    #[inline(always)]
    pub const fn is_positive(self) -> bool {
        !self.is_zero() && matches!(self.sign(), Sign::Positive)
    }

    /// Returns `true` if `self` is negative and `false` if the number is zero
    /// or positive
    #[inline(always)]
    pub const fn is_negative(self) -> bool {
        matches!(self.sign(), Sign::Negative)
    }

    /// Returns the number of ones in the binary representation of `self`.
    #[inline(always)]
    pub fn count_ones(&self) -> usize {
        self.0.count_ones()
    }

    /// Returns the number of zeros in the binary representation of `self`.
    #[inline(always)]
    pub fn count_zeros(&self) -> usize {
        self.0.count_zeros()
    }

    /// Returns the number of leading zeros in the binary representation of
    /// `self`.
    #[inline(always)]
    pub fn leading_zeros(&self) -> usize {
        self.0.leading_zeros()
    }

    /// Returns the number of leading zeros in the binary representation of
    /// `self`.
    #[inline(always)]
    pub fn trailing_zeros(&self) -> usize {
        self.0.trailing_zeros()
    }

    /// Returns the number of leading ones in the binary representation of
    /// `self`.
    #[inline(always)]
    pub fn trailing_ones(&self) -> usize {
        self.0.trailing_ones()
    }

    /// Return if specific bit is set.
    ///
    /// # Panics
    ///
    /// If index exceeds the bit width of the number.
    #[inline(always)]
    #[track_caller]
    pub const fn bit(&self, index: usize) -> bool {
        self.0.bit(index)
    }

    /// Return specific byte.
    ///
    /// # Panics
    ///
    /// If index exceeds the byte width of the number.
    #[inline(always)]
    #[track_caller]
    pub const fn byte(&self, index: usize) -> u8 {
        let limbs = self.0.as_limbs();
        match index {
            0..=7 => limbs[3].to_be_bytes()[index],
            8..=15 => limbs[2].to_be_bytes()[index - 8],
            16..=23 => limbs[1].to_be_bytes()[index - 16],
            24..=31 => limbs[0].to_be_bytes()[index - 24],
            _ => panic!(),
        }
    }

    /// Return the least number of bits needed to represent the number
    #[inline(always)]
    pub fn bits(self) -> u32 {
        let unsigned = self.unsigned_abs();
        let unsigned_bits = unsigned.bit_len();

        // NOTE: We need to deal with two special cases:
        //   - the number is 0
        //   - the number is a negative power of `2`. These numbers are written as `0b11..1100..00`.
        //   In the case of a negative power of two, the number of bits required
        //   to represent the negative signed value is equal to the number of
        //   bits required to represent its absolute value as an unsigned
        //   integer. This is best illustrated by an example: the number of bits
        //   required to represent `-128` is `8` since it is equal to `i8::MIN`
        //   and, therefore, obviously fits in `8` bits. This is equal to the
        //   number of bits required to represent `128` as an unsigned integer
        //   (which fits in a `u8`).  However, the number of bits required to
        //   represent `128` as a signed integer is `9`, as it is greater than
        //   `i8::MAX`.  In the general case, an extra bit is needed to
        //   represent the sign.
        let bits = if self.count_zeros() == self.trailing_zeros() {
            // `self` is zero or a negative power of two
            unsigned_bits
        } else {
            unsigned_bits + 1
        };

        bits as _
    }

    /// Creates a `Signed` from a sign and an absolute value. Returns the value
    /// and a bool that is true if the conversion caused an overflow.
    #[inline(always)]
    pub fn overflowing_from_sign_and_abs(sign: Sign, abs: Uint<BITS, LIMBS>) -> (Self, bool) {
        let value = Self(match sign {
            Sign::Positive => abs,
            Sign::Negative => twos_complement(abs),
        });

        (value, value.sign() != sign)
    }

    /// Creates a `Signed` from an absolute value and a negative flag. Returns
    /// `None` if it would overflow as `Signed`.
    #[inline(always)]
    pub fn checked_from_sign_and_abs(sign: Sign, abs: Uint<BITS, LIMBS>) -> Option<Self> {
        let (result, overflow) = Self::overflowing_from_sign_and_abs(sign, abs);
        if overflow {
            None
        } else {
            Some(result)
        }
    }

    /// Convert from a decimal string.
    pub fn from_dec_str(value: &str) -> Result<Self, errors::ParseSignedError> {
        let (sign, value) = match value.as_bytes().first() {
            Some(b'+') => (Sign::Positive, &value[1..]),
            Some(b'-') => (Sign::Negative, &value[1..]),
            _ => (Sign::Positive, value),
        };
        let abs = Uint::<BITS, LIMBS>::from_str_radix(value, 10)?;
        Self::checked_from_sign_and_abs(sign, abs).ok_or(errors::ParseSignedError::IntegerOverflow)
    }

    /// Convert to a decimal string.
    pub fn to_dec_string(self) -> String {
        let sign = self.sign();
        let abs = self.unsigned_abs();

        format!("{sign}{abs}")
    }

    /// Convert from a hex string.
    pub fn from_hex_str(value: &str) -> Result<Self, errors::ParseSignedError> {
        let (sign, value) = match value.as_bytes().first() {
            Some(b'+') => (Sign::Positive, &value[1..]),
            Some(b'-') => (Sign::Negative, &value[1..]),
            _ => (Sign::Positive, value),
        };

        let value = value.strip_prefix("0x").unwrap_or(value);

        if value.len() > 64 {
            return Err(errors::ParseSignedError::IntegerOverflow);
        }

        let abs = Uint::<BITS, LIMBS>::from_str_radix(value, 16)?;
        Self::checked_from_sign_and_abs(sign, abs).ok_or(errors::ParseSignedError::IntegerOverflow)
    }

    /// Convert to a hex string.
    pub fn to_hex_string(self) -> String {
        let sign = self.sign();
        let abs = self.unsigned_abs();

        format!("{sign}0x{abs:x}")
    }

    /// Splits a Signed into its absolute value and negative flag.
    #[inline(always)]
    pub fn into_sign_and_abs(self) -> (Sign, Uint<BITS, LIMBS>) {
        let sign = self.sign();
        let abs = match sign {
            Sign::Positive => self.0,
            Sign::Negative => twos_complement(self.0),
        };
        (sign, abs)
    }

    /// Convert to a slice in BE format
    ///
    /// # Panics
    ///
    /// If the given slice is not exactly 32 bytes long.
    #[inline(always)]
    #[track_caller]
    pub fn to_be_bytes(self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    /// Convert to a slice in LE format
    ///
    /// # Panics
    ///
    /// If the given slice is not exactly 32 bytes long.
    #[inline(always)]
    #[track_caller]
    pub fn to_le_bytes(self) -> [u8; 32] {
        self.0.to_le_bytes()
    }

    /// Convert from an array in BE format
    ///
    /// # Panics
    ///
    /// If the given array is not the correct length.
    #[inline(always)]
    #[track_caller]
    pub fn from_be_bytes<const BYTES: usize>(bytes: [u8; BYTES]) -> Self {
        Self(Uint::from_be_bytes::<BYTES>(bytes))
    }

    /// Convert from an array in LE format
    ///
    /// # Panics
    ///
    /// If the given array is not the correct length.
    #[inline(always)]
    #[track_caller]
    pub fn from_le_bytes<const BYTES: usize>(bytes: [u8; BYTES]) -> Self {
        Self(Uint::from_le_bytes::<BYTES>(bytes))
    }

    /// Convert from a slice in BE format
    pub fn try_from_be_slice(slice: &[u8]) -> Option<Self> {
        Some(Self(Uint::try_from_be_slice(slice)?))
    }

    /// Convert from a slice in LE format
    pub fn try_from_le_slice(slice: &[u8]) -> Option<Self> {
        Some(Self(Uint::try_from_le_slice(slice)?))
    }

    /// Get a reference to the underlying limbs
    pub const fn as_limbs(&self) -> &[u64; LIMBS] {
        self.0.as_limbs()
    }

    /// Get the underlying limbs
    pub const fn into_limbs(self) -> [u64; LIMBS] {
        self.0.into_limbs()
    }

    /// Instantiate from limbs
    pub const fn from_limbs(limbs: [u64; LIMBS]) -> Self {
        Self(Uint::from_limbs(limbs))
    }
}

#[cfg(feature = "serde")]
impl<const BITS: usize, const LIMBS: usize> serde::Serialize for Signed<BITS, LIMBS> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de, const BITS: usize, const LIMBS: usize> serde::Deserialize<'de> for Signed<BITS, LIMBS> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use ruint::{
        aliases::{U0, U1, U128, U160, U192, U256},
        BaseConvertError, ParseError,
    };
    // use serde_json::json;
    use std::ops::Neg;

    use super::*;
    use crate::{
        aliases::{I0, I1, I128, I160, I192, I256},
        BigIntConversionError, ParseSignedError,
    };

    // type U2 = Uint<2, 1>;
    type I96 = Signed<96, 2>;
    type U96 = Uint<96, 2>;

    #[test]
    fn identities() {
        macro_rules! test_identities {
            ($signed:ty, $max:literal, $min:literal) => {
                assert_eq!(<$signed>::zero().to_string(), "0");
                assert_eq!(<$signed>::one().to_string(), "1");
                assert_eq!(<$signed>::minus_one().to_string(), "-1");
                assert_eq!(<$signed>::max_value().to_string(), $max);
                assert_eq!(<$signed>::min_value().to_string(), $min);
            };
        }

        assert_eq!(I0::zero().to_string(), "0");
        assert_eq!(I1::zero().to_string(), "0");
        assert_eq!(I1::one().to_string(), "-1");

        test_identities!(
            I96,
            "39614081257132168796771975167",
            "-39614081257132168796771975168"
        );
        test_identities!(
            I128,
            "170141183460469231731687303715884105727",
            "-170141183460469231731687303715884105728"
        );
        test_identities!(
            I192,
            "3138550867693340381917894711603833208051177722232017256447",
            "-3138550867693340381917894711603833208051177722232017256448"
        );
        test_identities!(
            I256,
            "57896044618658097711785492504343953926634992332820282019728792003956564819967",
            "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
        );
    }

    #[test]
    // #[allow(clippy::cognitive_complexity)]
    fn std_num_conversion() {
        // test conversion from basic types

        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty, $i:ty, $u:ty) => {
                // Test a specific number
                assert_eq!(<$i_struct>::try_from(-42 as $i).unwrap().to_string(), "-42");
                assert_eq!(<$i_struct>::try_from(42 as $i).unwrap().to_string(), "42");
                assert_eq!(<$i_struct>::try_from(42 as $u).unwrap().to_string(), "42");

                if <$u_struct>::BITS as u32 >= <$u>::BITS {
                    assert_eq!(
                        <$i_struct>::try_from(<$i>::MAX).unwrap().to_string(),
                        <$i>::MAX.to_string(),
                    );
                    assert_eq!(
                        <$i_struct>::try_from(<$i>::MIN).unwrap().to_string(),
                        <$i>::MIN.to_string(),
                    );
                } else {
                    assert_eq!(
                        <$i_struct>::try_from(<$i>::MAX).unwrap_err(),
                        BigIntConversionError,
                    );
                }
            };

            ($i_struct:ty, $u_struct:ty) => {
                run_test!($i_struct, $u_struct, i8, u8);
                run_test!($i_struct, $u_struct, i16, u16);
                run_test!($i_struct, $u_struct, i32, u32);
                run_test!($i_struct, $u_struct, i64, u64);
                run_test!($i_struct, $u_struct, i128, u128);
                run_test!($i_struct, $u_struct, isize, usize);
            };
        }

        // edge cases
        assert_eq!(I0::unchecked_from(0), I0::default());
        assert_eq!(I0::try_from(1u8), Err(BigIntConversionError));
        assert_eq!(I0::try_from(1i8), Err(BigIntConversionError));
        assert_eq!(I1::unchecked_from(0), I1::default());
        assert_eq!(I1::try_from(1u8), Err(BigIntConversionError));
        assert_eq!(I1::try_from(1i8), Err(BigIntConversionError));
        assert_eq!(I1::try_from(-1), Ok(I1::MINUS_ONE));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn from_dec_str() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let min_abs: $u_struct = <$i_struct>::MIN.0;
                let unsigned = <$u_struct>::from_str_radix("3141592653589793", 10).unwrap();

                let value = <$i_struct>::from_dec_str(&format!("-{unsigned}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Negative, unsigned));

                let value = <$i_struct>::from_dec_str(&format!("{unsigned}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Positive, unsigned));

                let value = <$i_struct>::from_dec_str(&format!("+{unsigned}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Positive, unsigned));

                let err = <$i_struct>::from_dec_str("invalid string").unwrap_err();
                assert_eq!(
                    err,
                    ParseSignedError::Ruint(ParseError::BaseConvertError(
                        BaseConvertError::InvalidDigit(18, 10)
                    ))
                );

                let err = <$i_struct>::from_dec_str(&format!("1{}", <$u_struct>::MAX)).unwrap_err();
                assert_eq!(err, ParseSignedError::IntegerOverflow);

                let err = <$i_struct>::from_dec_str(&format!("-{}", <$u_struct>::MAX)).unwrap_err();
                assert_eq!(err, ParseSignedError::IntegerOverflow);

                let value = <$i_struct>::from_dec_str(&format!("-{}", min_abs)).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Negative, min_abs));

                let err = <$i_struct>::from_dec_str(&format!("{}", min_abs)).unwrap_err();
                assert_eq!(err, ParseSignedError::IntegerOverflow);
            };
        }

        assert_eq!(I0::from_dec_str("0"), Ok(I0::default()));
        assert_eq!(I1::from_dec_str("0"), Ok(I1::ZERO));
        assert_eq!(I1::from_dec_str("-1"), Ok(I1::MINUS_ONE));
        assert_eq!(
            I1::from_dec_str("1"),
            Err(ParseSignedError::IntegerOverflow)
        );

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn from_hex_str() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let min_abs = <$i_struct>::MIN.0;
                let unsigned = <$u_struct>::from_str_radix("3141592653589793", 10).unwrap();

                let value = <$i_struct>::from_hex_str(&format!("-{unsigned:x}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Negative, unsigned));

                let value = <$i_struct>::from_hex_str(&format!("-0x{unsigned:x}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Negative, unsigned));

                let value = <$i_struct>::from_hex_str(&format!("{unsigned:x}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Positive, unsigned));

                let value = <$i_struct>::from_hex_str(&format!("0x{unsigned:x}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Positive, unsigned));

                let value = <$i_struct>::from_hex_str(&format!("+0x{unsigned:x}")).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Positive, unsigned));

                let err = <$i_struct>::from_hex_str("invalid string").unwrap_err();
                assert!(matches!(err, ParseSignedError::Ruint(_)));

                let err =
                    <$i_struct>::from_hex_str(&format!("1{:x}", <$u_struct>::MAX)).unwrap_err();
                assert!(matches!(err, ParseSignedError::IntegerOverflow));

                let err =
                    <$i_struct>::from_hex_str(&format!("-{:x}", <$u_struct>::MAX)).unwrap_err();
                assert!(matches!(err, ParseSignedError::IntegerOverflow));

                let value = <$i_struct>::from_hex_str(&format!("-{:x}", min_abs)).unwrap();
                assert_eq!(value.into_sign_and_abs(), (Sign::Negative, min_abs));

                let err = <$i_struct>::from_hex_str(&format!("{:x}", min_abs)).unwrap_err();
                assert!(matches!(err, ParseSignedError::IntegerOverflow));
            };
        }

        assert_eq!(I0::from_hex_str("0x0"), Ok(I0::default()));
        assert_eq!(I1::from_hex_str("0x0"), Ok(I1::ZERO));
        assert_eq!(I1::from_hex_str("0x0"), Ok(I1::ZERO));
        assert_eq!(I1::from_hex_str("-0x1"), Ok(I1::MINUS_ONE));
        assert_eq!(
            I1::from_hex_str("0x1"),
            Err(ParseSignedError::IntegerOverflow)
        );

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn parse() {
        assert_eq!("0x0".parse::<I0>(), Ok(I0::default()));
        assert_eq!("+0x0".parse::<I0>(), Ok(I0::default()));
        assert_eq!("0x0".parse::<I1>(), Ok(I1::ZERO));
        assert_eq!("+0x0".parse::<I1>(), Ok(I1::ZERO));
        assert_eq!("-0x1".parse::<I1>(), Ok(I1::MINUS_ONE));
        assert_eq!("0x1".parse::<I1>(), Err(ParseSignedError::IntegerOverflow));

        assert_eq!("0".parse::<I0>(), Ok(I0::default()));
        assert_eq!("+0".parse::<I0>(), Ok(I0::default()));
        assert_eq!("0".parse::<I1>(), Ok(I1::ZERO));
        assert_eq!("+0".parse::<I1>(), Ok(I1::ZERO));
        assert_eq!("-1".parse::<I1>(), Ok(I1::MINUS_ONE));
        assert_eq!("1".parse::<I1>(), Err(ParseSignedError::IntegerOverflow));
    }

    #[test]
    fn formatting() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let unsigned = <$u_struct>::from_str_radix("3141592653589793", 10).unwrap();
                let positive = <$i_struct>::try_from(unsigned).unwrap();
                let negative = -positive;

                assert_eq!(format!("{positive}"), format!("{unsigned}"));
                assert_eq!(format!("{negative}"), format!("-{unsigned}"));
                assert_eq!(format!("{positive:+}"), format!("+{unsigned}"));
                assert_eq!(format!("{negative:+}"), format!("-{unsigned}"));

                assert_eq!(format!("{positive:x}"), format!("{unsigned:x}"));
                assert_eq!(format!("{negative:x}"), format!("-{unsigned:x}"));
                assert_eq!(format!("{positive:+x}"), format!("+{unsigned:x}"));
                assert_eq!(format!("{negative:+x}"), format!("-{unsigned:x}"));

                assert_eq!(
                    format!("{positive:X}"),
                    format!("{unsigned:x}").to_uppercase()
                );
                assert_eq!(
                    format!("{negative:X}"),
                    format!("-{unsigned:x}").to_uppercase()
                );
                assert_eq!(
                    format!("{positive:+X}"),
                    format!("+{unsigned:x}").to_uppercase()
                );
                assert_eq!(
                    format!("{negative:+X}"),
                    format!("-{unsigned:x}").to_uppercase()
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(format!("{z} {o} {m}"), "0 0 -1");

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn signs() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(<$i_struct>::MAX.sign(), Sign::Positive);
                assert!(<$i_struct>::MAX.is_positive());
                assert!(!<$i_struct>::MAX.is_negative());
                assert!(!<$i_struct>::MAX.is_zero());

                assert_eq!(<$i_struct>::one().sign(), Sign::Positive);
                assert!(<$i_struct>::one().is_positive());
                assert!(!<$i_struct>::one().is_negative());
                assert!(!<$i_struct>::one().is_zero());

                assert_eq!(<$i_struct>::MIN.sign(), Sign::Negative);
                assert!(!<$i_struct>::MIN.is_positive());
                assert!(<$i_struct>::MIN.is_negative());
                assert!(!<$i_struct>::MIN.is_zero());

                assert_eq!(<$i_struct>::minus_one().sign(), Sign::Negative);
                assert!(!<$i_struct>::minus_one().is_positive());
                assert!(<$i_struct>::minus_one().is_negative());
                assert!(!<$i_struct>::minus_one().is_zero());

                assert_eq!(<$i_struct>::zero().sign(), Sign::Positive);
                assert!(!<$i_struct>::zero().is_positive());
                assert!(!<$i_struct>::zero().is_negative());
                assert!(<$i_struct>::zero().is_zero());
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.sign(), Sign::Positive);
        assert_eq!(o.sign(), Sign::Positive);
        assert_eq!(m.sign(), Sign::Negative);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn abs() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let positive = <$i_struct>::from_dec_str("3141592653589793").unwrap();
                let negative = <$i_struct>::from_dec_str("-27182818284590").unwrap();

                assert_eq!(positive.sign(), Sign::Positive);
                assert_eq!(positive.abs().sign(), Sign::Positive);
                assert_eq!(positive, positive.abs());
                assert_ne!(negative, negative.abs());
                assert_eq!(negative.sign(), Sign::Negative);
                assert_eq!(negative.abs().sign(), Sign::Positive);
                assert_eq!(<$i_struct>::zero().abs(), <$i_struct>::zero());
                assert_eq!(<$i_struct>::MAX.abs(), <$i_struct>::MAX);
                assert_eq!((-<$i_struct>::MAX).abs(), <$i_struct>::MAX);
                assert_eq!(<$i_struct>::MIN.checked_abs(), None);
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.abs(), z);
        assert_eq!(o.abs(), o);
        assert_eq!(m.checked_abs(), None);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn neg() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let positive = <$i_struct>::from_dec_str("3141592653589793")
                    .unwrap()
                    .sign();
                let negative = -positive;

                assert_eq!(-positive, negative);
                assert_eq!(-negative, positive);

                assert_eq!(-<$i_struct>::zero(), <$i_struct>::zero());
                assert_eq!(-(-<$i_struct>::MAX), <$i_struct>::MAX);
                assert_eq!(<$i_struct>::MIN.checked_neg(), None);
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(-z, z);
        assert_eq!(-o, o);
        assert_eq!(m.checked_neg(), None);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn bits() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(<$i_struct>::try_from(0b1000).unwrap().bits(), 5);
                assert_eq!(<$i_struct>::try_from(-0b1000).unwrap().bits(), 4);

                assert_eq!(<$i_struct>::try_from(i64::MAX).unwrap().bits(), 64);
                assert_eq!(<$i_struct>::try_from(i64::MIN).unwrap().bits(), 64);

                assert_eq!(<$i_struct>::MAX.bits(), <$i_struct>::BITS as u32);
                assert_eq!(<$i_struct>::MIN.bits(), <$i_struct>::BITS as u32);

                assert_eq!(<$i_struct>::zero().bits(), 0);
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.bits(), 0);
        assert_eq!(o.bits(), 0);
        assert_eq!(m.bits(), 1);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn bit_shift() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    <$i_struct>::one() << <$i_struct>::BITS - 1,
                    <$i_struct>::MIN
                );
                assert_eq!(
                    <$i_struct>::MIN >> <$i_struct>::BITS - 1,
                    <$i_struct>::one()
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z << 1, z >> 1);
        assert_eq!(o << 1, o >> 0);
        assert_eq!(m << 1, o);
        assert_eq!(m >> 1, o);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn arithmetic_shift_right() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let exp = <$i_struct>::BITS - 2;
                let shift = <$i_struct>::BITS - 3;

                let value =
                    <$i_struct>::from_raw(<$u_struct>::from(2u8).pow(<$u_struct>::from(exp))).neg();

                let expected_result =
                    <$i_struct>::from_raw(<$u_struct>::MAX - <$u_struct>::from(1u8));
                assert_eq!(
                    value.asr(shift),
                    expected_result,
                    "1011...1111 >> 253 was not 1111...1110"
                );

                let value = <$i_struct>::minus_one();
                let expected_result = <$i_struct>::minus_one();
                assert_eq!(
                    value.asr(250),
                    expected_result,
                    "-1 >> any_amount was not -1"
                );

                let value = <$i_struct>::from_raw(
                    <$u_struct>::from(2u8).pow(<$u_struct>::from(<$i_struct>::BITS - 2)),
                )
                .neg();
                let expected_result = <$i_struct>::minus_one();
                assert_eq!(
                    value.asr(<$i_struct>::BITS - 1),
                    expected_result,
                    "1011...1111 >> 255 was not -1"
                );

                let value = <$i_struct>::from_raw(
                    <$u_struct>::from(2u8).pow(<$u_struct>::from(<$i_struct>::BITS - 2)),
                )
                .neg();
                let expected_result = <$i_struct>::minus_one();
                assert_eq!(
                    value.asr(1024),
                    expected_result,
                    "1011...1111 >> 1024 was not -1"
                );

                let value = <$i_struct>::try_from(1024i32).unwrap();
                let expected_result = <$i_struct>::try_from(32i32).unwrap();
                assert_eq!(value.asr(5), expected_result, "1024 >> 5 was not 32");

                let value = <$i_struct>::MAX;
                let expected_result = <$i_struct>::zero();
                assert_eq!(
                    value.asr(255),
                    expected_result,
                    "<$i_struct>::MAX >> 255 was not 0"
                );

                let value =
                    <$i_struct>::from_raw(<$u_struct>::from(2u8).pow(<$u_struct>::from(exp))).neg();
                let expected_result = value;
                assert_eq!(
                    value.asr(0),
                    expected_result,
                    "1011...1111 >> 0 was not 1011...111"
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.asr(1), z);
        assert_eq!(o.asr(1), o);
        assert_eq!(m.asr(1), m);
        assert_eq!(m.asr(1000), m);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn arithmetic_shift_left() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let value = <$i_struct>::minus_one();
                let expected_result = Some(value);
                assert_eq!(value.asl(0), expected_result, "-1 << 0 was not -1");

                let value = <$i_struct>::minus_one();
                let expected_result = None;
                assert_eq!(
                    value.asl(256),
                    expected_result,
                    "-1 << 256 did not overflow (result should be 0000...0000)"
                );

                let value = <$i_struct>::minus_one();
                let expected_result = Some(<$i_struct>::from_raw(
                    <$u_struct>::from(2u8).pow(<$u_struct>::from(<$i_struct>::BITS - 1)),
                ));
                assert_eq!(
                    value.asl(<$i_struct>::BITS - 1),
                    expected_result,
                    "-1 << 255 was not 1000...0000"
                );

                let value = <$i_struct>::try_from(-1024i32).unwrap();
                let expected_result = Some(<$i_struct>::try_from(-32768i32).unwrap());
                assert_eq!(value.asl(5), expected_result, "-1024 << 5 was not -32768");

                let value = <$i_struct>::try_from(1024i32).unwrap();
                let expected_result = Some(<$i_struct>::try_from(32768i32).unwrap());
                assert_eq!(value.asl(5), expected_result, "1024 << 5 was not 32768");

                let value = <$i_struct>::try_from(1024i32).unwrap();
                let expected_result = None;
                assert_eq!(
                    value.asl(<$i_struct>::BITS - 11),
                    expected_result,
                    "1024 << 245 did not overflow (result should be 1000...0000)"
                );

                let value = <$i_struct>::zero();
                let expected_result = Some(value);
                assert_eq!(value.asl(1024), expected_result, "0 << anything was not 0");
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.asl(1), Some(z));
        assert_eq!(o.asl(1), Some(o));
        assert_eq!(m.asl(1), None);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn addition() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    <$i_struct>::MIN.overflowing_add(<$i_struct>::MIN),
                    (<$i_struct>::zero(), true)
                );
                assert_eq!(
                    <$i_struct>::MAX.overflowing_add(<$i_struct>::MAX),
                    (<$i_struct>::try_from(-2).unwrap(), true)
                );

                assert_eq!(
                    <$i_struct>::MIN.overflowing_add(<$i_struct>::minus_one()),
                    (<$i_struct>::MAX, true)
                );
                assert_eq!(
                    <$i_struct>::MAX.overflowing_add(<$i_struct>::one()),
                    (<$i_struct>::MIN, true)
                );

                assert_eq!(
                    <$i_struct>::MAX + <$i_struct>::MIN,
                    <$i_struct>::minus_one()
                );
                assert_eq!(
                    <$i_struct>::try_from(2).unwrap() + <$i_struct>::try_from(40).unwrap(),
                    <$i_struct>::try_from(42).unwrap()
                );

                assert_eq!(
                    <$i_struct>::zero() + <$i_struct>::zero(),
                    <$i_struct>::zero()
                );

                assert_eq!(
                    <$i_struct>::MAX.saturating_add(<$i_struct>::MAX),
                    <$i_struct>::MAX
                );
                assert_eq!(
                    <$i_struct>::MIN.saturating_add(<$i_struct>::minus_one()),
                    <$i_struct>::MIN
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z + z, z);
        assert_eq!(o + o, o);
        assert_eq!(m + o, m);
        assert_eq!(m.overflowing_add(m), (o, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn subtraction() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    <$i_struct>::MIN.overflowing_sub(<$i_struct>::MAX),
                    (<$i_struct>::one(), true)
                );
                assert_eq!(
                    <$i_struct>::MAX.overflowing_sub(<$i_struct>::MIN),
                    (<$i_struct>::minus_one(), true)
                );

                assert_eq!(
                    <$i_struct>::MIN.overflowing_sub(<$i_struct>::one()),
                    (<$i_struct>::MAX, true)
                );
                assert_eq!(
                    <$i_struct>::MAX.overflowing_sub(<$i_struct>::minus_one()),
                    (<$i_struct>::MIN, true)
                );

                assert_eq!(
                    <$i_struct>::zero().overflowing_sub(<$i_struct>::MIN),
                    (<$i_struct>::MIN, true)
                );

                assert_eq!(<$i_struct>::MAX - <$i_struct>::MAX, <$i_struct>::zero());
                assert_eq!(
                    <$i_struct>::try_from(2).unwrap() - <$i_struct>::try_from(44).unwrap(),
                    <$i_struct>::try_from(-42).unwrap()
                );

                assert_eq!(
                    <$i_struct>::zero() - <$i_struct>::zero(),
                    <$i_struct>::zero()
                );

                assert_eq!(
                    <$i_struct>::MAX.saturating_sub(<$i_struct>::MIN),
                    <$i_struct>::MAX
                );
                assert_eq!(
                    <$i_struct>::MIN.saturating_sub(<$i_struct>::one()),
                    <$i_struct>::MIN
                );
            };
        }

        // run_test!(I0, U0);
        // run_test!(I1, U1);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn multiplication() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    <$i_struct>::MIN.overflowing_mul(<$i_struct>::MAX),
                    (<$i_struct>::MIN, true)
                );
                assert_eq!(
                    <$i_struct>::MAX.overflowing_mul(<$i_struct>::MIN),
                    (<$i_struct>::MIN, true)
                );

                assert_eq!(<$i_struct>::MIN * <$i_struct>::one(), <$i_struct>::MIN);
                assert_eq!(
                    <$i_struct>::try_from(2).unwrap() * <$i_struct>::try_from(-21).unwrap(),
                    <$i_struct>::try_from(-42).unwrap()
                );

                assert_eq!(
                    <$i_struct>::MAX.saturating_mul(<$i_struct>::MAX),
                    <$i_struct>::MAX
                );
                assert_eq!(
                    <$i_struct>::MAX.saturating_mul(<$i_struct>::try_from(2).unwrap()),
                    <$i_struct>::MAX
                );
                assert_eq!(
                    <$i_struct>::MIN.saturating_mul(<$i_struct>::try_from(-2).unwrap()),
                    <$i_struct>::MAX
                );

                assert_eq!(
                    <$i_struct>::MIN.saturating_mul(<$i_struct>::MAX),
                    <$i_struct>::MIN
                );
                assert_eq!(
                    <$i_struct>::MIN.saturating_mul(<$i_struct>::try_from(2).unwrap()),
                    <$i_struct>::MIN
                );
                assert_eq!(
                    <$i_struct>::MAX.saturating_mul(<$i_struct>::try_from(-2).unwrap()),
                    <$i_struct>::MIN
                );

                assert_eq!(
                    <$i_struct>::zero() * <$i_struct>::zero(),
                    <$i_struct>::zero()
                );
                assert_eq!(
                    <$i_struct>::one() * <$i_struct>::zero(),
                    <$i_struct>::zero()
                );
                assert_eq!(<$i_struct>::MAX * <$i_struct>::zero(), <$i_struct>::zero());
                assert_eq!(<$i_struct>::MIN * <$i_struct>::zero(), <$i_struct>::zero());
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z * z, z);
        assert_eq!(o * o, o);
        assert_eq!(m * o, o);
        assert_eq!(m.overflowing_mul(m), (m, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn division() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                // The only case for overflow.
                assert_eq!(
                    <$i_struct>::MIN.overflowing_div(<$i_struct>::try_from(-1).unwrap()),
                    (<$i_struct>::MIN, true)
                );

                assert_eq!(
                    <$i_struct>::MIN / <$i_struct>::MAX,
                    <$i_struct>::try_from(-1).unwrap()
                );
                assert_eq!(<$i_struct>::MAX / <$i_struct>::MIN, <$i_struct>::zero());

                assert_eq!(<$i_struct>::MIN / <$i_struct>::one(), <$i_struct>::MIN);
                assert_eq!(
                    <$i_struct>::try_from(-42).unwrap() / <$i_struct>::try_from(-21).unwrap(),
                    <$i_struct>::try_from(2).unwrap()
                );
                assert_eq!(
                    <$i_struct>::try_from(-42).unwrap() / <$i_struct>::try_from(2).unwrap(),
                    <$i_struct>::try_from(-21).unwrap()
                );
                assert_eq!(
                    <$i_struct>::try_from(42).unwrap() / <$i_struct>::try_from(-21).unwrap(),
                    <$i_struct>::try_from(-2).unwrap()
                );
                assert_eq!(
                    <$i_struct>::try_from(42).unwrap() / <$i_struct>::try_from(21).unwrap(),
                    <$i_struct>::try_from(2).unwrap()
                );

                // The only saturating corner case.
                assert_eq!(
                    <$i_struct>::MIN.saturating_div(<$i_struct>::try_from(-1).unwrap()),
                    <$i_struct>::MAX
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.checked_div(z), None);
        assert_eq!(o.checked_div(o), None);
        assert_eq!(m.checked_div(o), None);
        assert_eq!(m.overflowing_div(m), (m, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn division_by_zero() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let err = std::panic::catch_unwind(|| {
                    let _ = <$i_struct>::one() / <$i_struct>::zero();
                });
                assert!(err.is_err());
            };
        }

        run_test!(I0, U0);
        run_test!(I1, U1);
        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn div_euclid() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let a = <$i_struct>::try_from(7).unwrap();
                let b = <$i_struct>::try_from(4).unwrap();

                assert_eq!(a.div_euclid(b), <$i_struct>::one()); // 7 >= 4 * 1
                assert_eq!(a.div_euclid(-b), <$i_struct>::minus_one()); // 7 >= -4 * -1
                assert_eq!((-a).div_euclid(b), -<$i_struct>::try_from(2).unwrap()); // -7 >= 4 * -2
                assert_eq!((-a).div_euclid(-b), <$i_struct>::try_from(2).unwrap()); // -7 >= -4 * 2

                // Overflowing
                assert_eq!(
                    <$i_struct>::MIN.overflowing_div_euclid(<$i_struct>::minus_one()),
                    (<$i_struct>::MIN, true)
                );
                // Wrapping
                assert_eq!(
                    <$i_struct>::MIN.wrapping_div_euclid(<$i_struct>::minus_one()),
                    <$i_struct>::MIN
                );
                // // Checked
                assert_eq!(
                    <$i_struct>::MIN.checked_div_euclid(<$i_struct>::minus_one()),
                    None
                );
                assert_eq!(
                    <$i_struct>::one().checked_div_euclid(<$i_struct>::zero()),
                    None
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.checked_div_euclid(z), None);
        assert_eq!(o.checked_div_euclid(o), None);
        assert_eq!(m.checked_div_euclid(o), None);
        assert_eq!(m.overflowing_div_euclid(m), (m, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn rem_euclid() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let a = <$i_struct>::try_from(7).unwrap(); // or any other integer type
                let b = <$i_struct>::try_from(4).unwrap();

                assert_eq!(a.rem_euclid(b), <$i_struct>::try_from(3).unwrap());
                assert_eq!((-a).rem_euclid(b), <$i_struct>::one());
                assert_eq!(a.rem_euclid(-b), <$i_struct>::try_from(3).unwrap());
                assert_eq!((-a).rem_euclid(-b), <$i_struct>::one());

                // Overflowing
                assert_eq!(
                    a.overflowing_rem_euclid(b),
                    (<$i_struct>::try_from(3).unwrap(), false)
                );
                assert_eq!(
                    <$i_struct>::min_value().overflowing_rem_euclid(<$i_struct>::minus_one()),
                    (<$i_struct>::zero(), true)
                );

                // Wrapping
                assert_eq!(
                    <$i_struct>::try_from(100)
                        .unwrap()
                        .wrapping_rem_euclid(<$i_struct>::try_from(10).unwrap()),
                    <$i_struct>::zero()
                );
                assert_eq!(
                    <$i_struct>::min_value().wrapping_rem_euclid(<$i_struct>::minus_one()),
                    <$i_struct>::zero()
                );

                // Checked
                assert_eq!(
                    a.checked_rem_euclid(b),
                    Some(<$i_struct>::try_from(3).unwrap())
                );
                assert_eq!(a.checked_rem_euclid(<$i_struct>::zero()), None);
                assert_eq!(
                    <$i_struct>::min_value().checked_rem_euclid(<$i_struct>::minus_one()),
                    None
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.checked_rem_euclid(z), None);
        assert_eq!(o.checked_rem_euclid(o), None);
        assert_eq!(m.checked_rem_euclid(o), None);
        assert_eq!(m.overflowing_rem_euclid(m), (o, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn div_euclid_by_zero() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let err = std::panic::catch_unwind(|| {
                    let _ = <$i_struct>::one().div_euclid(<$i_struct>::zero());
                });
                assert!(err.is_err());

                let err = std::panic::catch_unwind(|| {
                    assert_eq!(
                        <$i_struct>::MIN.div_euclid(<$i_struct>::minus_one()),
                        <$i_struct>::MAX
                    );
                });
                assert!(err.is_err());
            };
        }

        run_test!(I0, U0);
        run_test!(I1, U1);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    #[should_panic]
    fn div_euclid_overflow() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let _ = <$i_struct>::MIN.div_euclid(<$i_struct>::minus_one());
            };
        }
        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn mod_by_zero() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                let err = std::panic::catch_unwind(|| {
                    let _ = <$i_struct>::one() % <$i_struct>::zero();
                });
                assert!(err.is_err());
            };
        }

        run_test!(I0, U0);
        run_test!(I1, U1);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn remainder() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                // The only case for overflow.
                assert_eq!(
                    <$i_struct>::MIN.overflowing_rem(<$i_struct>::try_from(-1).unwrap()),
                    (<$i_struct>::zero(), true)
                );
                assert_eq!(
                    <$i_struct>::try_from(-5).unwrap() % <$i_struct>::try_from(-2).unwrap(),
                    <$i_struct>::try_from(-1).unwrap()
                );
                assert_eq!(
                    <$i_struct>::try_from(5).unwrap() % <$i_struct>::try_from(-2).unwrap(),
                    <$i_struct>::one()
                );
                assert_eq!(
                    <$i_struct>::try_from(-5).unwrap() % <$i_struct>::try_from(2).unwrap(),
                    <$i_struct>::try_from(-1).unwrap()
                );
                assert_eq!(
                    <$i_struct>::try_from(5).unwrap() % <$i_struct>::try_from(2).unwrap(),
                    <$i_struct>::one()
                );

                assert_eq!(
                    <$i_struct>::MIN.checked_rem(<$i_struct>::try_from(-1).unwrap()),
                    None
                );
                assert_eq!(
                    <$i_struct>::one().checked_rem(<$i_struct>::one()),
                    Some(<$i_struct>::zero())
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.checked_rem(z), None);
        assert_eq!(o.checked_rem(o), None);
        assert_eq!(m.checked_rem(o), None);
        assert_eq!(m.overflowing_rem(m), (o, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn exponentiation() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    <$i_struct>::unchecked_from(1000).saturating_pow(<$u_struct>::from(1000)),
                    <$i_struct>::MAX
                );
                assert_eq!(
                    <$i_struct>::unchecked_from(-1000).saturating_pow(<$u_struct>::from(1001)),
                    <$i_struct>::MIN
                );

                assert_eq!(
                    <$i_struct>::unchecked_from(2).pow(<$u_struct>::from(64)),
                    <$i_struct>::unchecked_from(1u128 << 64)
                );
                assert_eq!(
                    <$i_struct>::unchecked_from(-2).pow(<$u_struct>::from(63)),
                    <$i_struct>::unchecked_from(i64::MIN)
                );

                assert_eq!(
                    <$i_struct>::zero().pow(<$u_struct>::from(42)),
                    <$i_struct>::zero()
                );
                assert_eq!(<$i_struct>::exp10(18).to_string(), "1000000000000000000");
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.pow(U0::default()), z);
        assert_eq!(o.overflowing_pow(U1::default()), (m, true));
        assert_eq!(o.overflowing_pow(U1::from(1u8)), (o, false));
        assert_eq!(m.overflowing_pow(U1::from(1u8)), (m, false));
        assert_eq!(m.overflowing_pow(U1::default()), (m, true));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn iterators() {
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_eq!(
                    (1..=5)
                        .map(<$i_struct>::try_from)
                        .map(Result::unwrap)
                        .sum::<$i_struct>(),
                    <$i_struct>::try_from(15).unwrap()
                );
                assert_eq!(
                    (1..=5)
                        .map(<$i_struct>::try_from)
                        .map(Result::unwrap)
                        .product::<$i_struct>(),
                    <$i_struct>::try_from(120).unwrap()
                );
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!([z; 0].into_iter().sum::<I0>(), z);
        assert_eq!([o; 1].into_iter().sum::<I1>(), o);
        assert_eq!([m; 1].into_iter().sum::<I1>(), m);

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }

    #[test]
    fn twos_complement() {
        macro_rules! assert_twos_complement {
            ($i_struct:ty, $u_struct:ty, $signed:ty, $unsigned:ty) => {
                if <$u_struct>::BITS as u32 >= <$unsigned>::BITS {
                    assert_eq!(
                        <$i_struct>::try_from(<$signed>::MAX)
                            .unwrap()
                            .twos_complement(),
                        <$u_struct>::try_from(<$signed>::MAX).unwrap()
                    );
                    assert_eq!(
                        <$i_struct>::try_from(<$signed>::MIN)
                            .unwrap()
                            .twos_complement(),
                        <$u_struct>::try_from(<$signed>::MIN.unsigned_abs()).unwrap()
                    );
                }

                assert_eq!(
                    <$i_struct>::try_from(0 as $signed)
                        .unwrap()
                        .twos_complement(),
                    <$u_struct>::try_from(0 as $signed).unwrap()
                );

                assert_eq!(
                    <$i_struct>::try_from(0 as $unsigned)
                        .unwrap()
                        .twos_complement(),
                    <$u_struct>::try_from(0 as $unsigned).unwrap()
                );
            };
        }
        macro_rules! run_test {
            ($i_struct:ty, $u_struct:ty) => {
                assert_twos_complement!($i_struct, $u_struct, i8, u8);
                assert_twos_complement!($i_struct, $u_struct, i16, u16);
                assert_twos_complement!($i_struct, $u_struct, i32, u32);
                assert_twos_complement!($i_struct, $u_struct, i64, u64);
                assert_twos_complement!($i_struct, $u_struct, i128, u128);
                assert_twos_complement!($i_struct, $u_struct, isize, usize);
            };
        }

        let z = I0::default();
        let o = I1::default();
        let m = I1::MINUS_ONE;
        assert_eq!(z.twos_complement(), U0::default());
        assert_eq!(o.twos_complement(), U1::default());
        assert_eq!(m.twos_complement(), U1::from(1));

        run_test!(I96, U96);
        run_test!(I128, U128);
        run_test!(I160, U160);
        run_test!(I192, U192);
        run_test!(I256, U256);
    }
}