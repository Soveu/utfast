use utfast::*;

/*
#[test]
fn test_bsr() {
    for i in 0..32 {
        let i = !(1u32 << i);
        assert_eq!(x86_leading_ones(i), i.leading_ones());
    }
}

#[test]
fn edge_bsr() {
    assert_eq!(x86_leading_ones(!0), (!0u32).leading_ones());
}
*/

#[test]
fn smol_test() {
    let bytes = "qwertyuiopasdfghjklzxcvbnm,.;'[]1234567890-=πœę©ß←↓→óþąśðæŋ’ə…łżźć„”ńµQWERTYUIOPASDFGHJKLZXCVBNM<>?L:{}|!@#$%^&*()_+ΩŒĘ®™¥↑↔ÓÞĄŚÐÆŊ•ƏŻŹĆ‘“Ń∞";
    let _ = check_utf8_v2(bytes.as_bytes()).unwrap();
}

fn xorshift(mut x: u32) -> u32 {
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    return x;
}
fn xorshift64(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x >> 17;
    return x;
}

#[test]
fn big_test() {
    let mut seed = 0x42;

    for _ in 0..10_000_000 {
        seed = xorshift(seed);
        let bytes = seed.to_ne_bytes();
        
        let std = std::str::from_utf8(&bytes);
        let this = check_utf8_v2(&bytes);

        if std.is_ok() && this.is_ok() {
            continue;
        }
        if std.is_err() && this.is_err() {
            let thiserr = this.unwrap_err();
            let stderr = std.unwrap_err();
            if stderr.valid_up_to() != thiserr {
                eprintln!("buffer: {:?}", bytes);
                eprintln!("stderr: {:?}", stderr);
                eprintln!("diserr: {:?}", thiserr);
                panic!();
            }
            continue;
        }
        
        eprintln!("buffer={:?}", bytes);
        panic!("std: {:?}\nthis: {:?}", std, this);
    }
}

