use num_bigint::BigInt;
use num_traits::One;
pub fn pow() {
    let mut _i = BigInt::one();
    loop {
        _i = &_i * 2;
    }
}
