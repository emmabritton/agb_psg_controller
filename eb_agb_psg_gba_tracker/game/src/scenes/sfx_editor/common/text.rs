use arrayvec::ArrayString;

pub const HEX: &[u8; 16] = b"0123456789ABCDEF";

pub fn push_num<const CAP: usize>(out: &mut ArrayString<CAP>, value: i32) {
    debug_assert!((-255..=255).contains(&value));
    if value < 0 {
        let _ = out.try_push('-');
    }
    let mut value = value.unsigned_abs().min(255) as u8;
    let mut started = false;
    for scale in [100u8, 10] {
        let mut digit = b'0';
        while value >= scale {
            value -= scale;
            digit += 1;
        }
        if digit != b'0' || started {
            let _ = out.try_push(digit as char);
            started = true;
        }
    }
    let _ = out.try_push((b'0' + value) as char);
}

pub fn push_opt_num<const CAP: usize>(out: &mut ArrayString<CAP>, value: Option<u8>) {
    match value {
        Some(value) => push_num(out, value as i32),
        None => {
            let _ = out.try_push('-');
        }
    }
}

pub fn push_fixed<const CAP: usize>(out: &mut ArrayString<CAP>, raw: u32, frac_bits: u32) {
    let mask = (1u32 << frac_bits) - 1;
    let mut whole = raw >> frac_bits;
    let mut tenths = ((raw & mask) * 10 + (1 << (frac_bits - 1))) >> frac_bits;
    if tenths == 10 {
        whole += 1;
        tenths = 0;
    }
    push_num(out, whole as i32);
    let _ = out.try_push('.');
    let _ = out.try_push((b'0' + tenths as u8) as char);
}
