use num_traits::cast::ToPrimitive;

#[derive(Clone, Debug)]
pub enum BigIntOrI64 {
    Int(i64),
    BigInt(num_bigint::BigInt),
}

impl PartialEq for BigIntOrI64 {
    fn eq(&self, other: &Self) -> bool {
        use BigIntOrI64::*;
        match (&self, &other) {
            (Int(i), Int(j)) => i == j,
            (Int(i), BigInt(b)) | (BigInt(b), Int(i)) => b == &num_bigint::BigInt::from(*i),
            (BigInt(a), BigInt(b)) => a == b,
        }
    }
}

impl Eq for BigIntOrI64 {}

/// A value holding JavaScript
/// [BigInt](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt) type
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    pub(crate) inner: BigIntOrI64,
}

impl BigInt {
    /// Return `Some` if value fits into `i64` and `None` otherwise
    pub fn as_i64(&self) -> Option<i64> {
        match &self.inner {
            BigIntOrI64::Int(int) => Some(*int),
            BigIntOrI64::BigInt(bigint) => bigint.to_i64(),
        }
    }
    /// Convert value into `num_bigint::BigInt`
    pub fn into_bigint(self) -> num_bigint::BigInt {
        match self.inner {
            BigIntOrI64::Int(int) => int.into(),
            BigIntOrI64::BigInt(bigint) => bigint,
        }
    }
}

impl std::fmt::Display for BigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.inner {
            BigIntOrI64::Int(i) => write!(f, "{}", i),
            BigIntOrI64::BigInt(ref i) => write!(f, "{}", i),
        }
    }
}

impl From<i64> for BigInt {
    fn from(int: i64) -> Self {
        BigInt {
            inner: BigIntOrI64::Int(int),
        }
    }
}

impl From<num_bigint::BigInt> for BigInt {
    fn from(bigint: num_bigint::BigInt) -> Self {
        BigInt {
            inner: BigIntOrI64::BigInt(bigint),
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_bigint_as_i64() {
        let value = BigInt {
            inner: BigIntOrI64::Int(1234i64),
        };
        assert_eq!(value.as_i64(), Some(1234i64));
    }

    #[test]
    fn test_bigint_as_i64_overflow() {
        let value = BigInt {
            inner: BigIntOrI64::BigInt(num_bigint::BigInt::from(std::i128::MAX)),
        };
        assert_eq!(value.as_i64(), None);
    }

    #[test]
    fn test_bigint_into_bigint() {
        for i in vec![
            0 as i128,
            std::i64::MAX as i128,
            std::i64::MIN as i128,
            std::i128::MAX,
            std::i128::MIN,
        ] {
            let value = BigInt {
                inner: BigIntOrI64::BigInt(num_bigint::BigInt::from(i)),
            };
            assert_eq!(value.into_bigint(), num_bigint::BigInt::from(i));
        }
    }
}
