#![feature(array_windows)]

/*
#![feature(asm)]
fn x86_leading_zeros(mut x: u32) -> u32 {
    unsafe {
        asm!("
            bsr {0:e}, {0:e}
            jnz wasnt_zero
            not {0:e}
            wasnt_zero:",
            inlateout(reg) x => x,
            options(nostack, nomem, pure)
        );
    }

    return 31u32.wrapping_sub(x);
}

fn x86_leading_ones(mut x: u32) -> u32 {
    return x86_leading_zeros(!x);
}
*/

pub fn check_utf8_v1(s: &[u8]) -> Result<(), usize> {
    let mut iter = s.array_windows::<4>();
    let mut bytes = 0u32;
    let mut stop = 0usize;

    while let Some(window) = iter.nth(bytes as usize) {
        stop += 1;
        bytes = 0;

        let dword = u32::from_le_bytes(*window);

        /* Note: LLVM knows u8::leading_ones can only return value from range (0..=8) */
        bytes = match (dword as u8).leading_ones().checked_sub(1) {
            Some(x) => x,
            None => continue, /* it's ascii */
        };

        let shift = match bytes.checked_sub(1) {
            Some(x) => x,
            None => return Err(stop-1), /* not valid for first byte */
        };

        let (char_range, shift) = match 2u32.checked_sub(shift) {
            Some(2) => (0x0080..0x0800, 24),
            Some(1) => (0x0800..0x10000, 16),
            Some(0) => (0x10000..0x110000, 8),
            None => return Err(stop-1), /* we can only accept 4 byte sequences */

            /* SAFETY: do the maffs, rustc */
            _ => unsafe { std::hint::unreachable_unchecked() },
        };

        /* We need the first byte, but we also need the later 3 bytes.
         * Solution: rotate, so we preserve first byte, but also prepare
         * the dword for masking */
        let dword = dword.rotate_right(8);
        let codepoint_mask = 0x3F_3F_3F_3Fu32.wrapping_shr(shift);
        let code = (dword & codepoint_mask).to_le_bytes();

        /* Assemble a char as it was a 4 byte utf8 */
        let character: u32 = ((code[2] as u32) << 0)
            | ((code[1] as u32) << 6)
            | ((code[0] as u32) << 12);

        /* Add the most significant bits */
        let first_byte_mask = 0xFFu32 >> (bytes+2);
        let first_byte = dword >> 24;
        let first_byte = first_byte & first_byte_mask;
        let character = character | (first_byte << 18);

        /* Shift back if there were fewer than 4 bytes */
        let character = character >> (3u32.wrapping_sub(bytes) * 6);

        let mask = 0xC0_C0_C0_C0u32.wrapping_shr(shift);
        let expect = 0x80_80_80_80u32.wrapping_shr(shift);

        /* Stack branches together, because why not */
        if dword & mask != expect {
            return Err(stop-1);
        }
        if !char_range.contains(&character) {
            return Err(stop-1);
        }
        if (0xD800..0xE000).contains(&character) {
            return Err(stop-1);
        }

        stop += bytes as usize;
    }

    /* TODO: maybe handle tail manually */
    /* TODO: this slice access is the only thing that panics in this function.
     * Also, when it is not panicking, it looks like it is moved upwards */
    let tail = unsafe { s.get_unchecked(stop..) };
    return match core::str::from_utf8(tail) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.valid_up_to() + stop),
    };
}

pub fn __check_utf8_v2(packed_codepoints: u32) -> Result<char, ()> {
    let first_byte = packed_codepoints as u8;

    /* TODO: check if returning instantly in case of ascii is faster */
    let bytes: u32 = match first_byte.leading_ones() {
        0 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        //1 => return Err(()), /* illegal for first byte */
        _ => return Err(()),
    };

    /* We rotate the bits, so we get the other codepoints in the front
     * and keep the first byte in the back */
    let packed_codepoints = packed_codepoints.rotate_right(8);

    let shift  = 4u32.wrapping_sub(bytes).wrapping_mul(8);

    /* Every byte after the first one has to begin with 0b10xxxxxx
     * 0xC0 catches the first two bytes, 0x80 is the expected result
     * of ANDing the codepoints and the mask */
    let mask   = 0xC0C0C0u32 >> shift;
    let expect = 0x808080u32 >> shift;

    /* The mask we use to extract actual codepoint value */
    let codepoint_bits_mask = 0x3F3F3Fu32;

    let code: u32 = packed_codepoints & codepoint_bits_mask;
    let code: [u8; 4] = code.to_le_bytes();

    let character: u32 = ((code[2] as u32) << 0)
        | ((code[1] as u32) << 6)
        | ((code[0] as u32) << 12);

    let first_byte_mask = 0xFFu32 >> bytes;
    let first_byte = (packed_codepoints >> 24) & first_byte_mask;
    let character = character | (first_byte << 18);

    /* Shift back the value to compensate for overshooting the length */
    let character = character >> (4u32.wrapping_sub(bytes) * 6);

    if packed_codepoints & mask != expect {
        return Err(());
    }

    /* It checks if character <= char::MAX && (0xD800..=0xDFFF).contains(character) */
    let character = match char::from_u32(character) {
        Some(x) => x,
        None => return Err(()),
    };

    /* UTF-8 characters must be encoded using the least amount of bytes possible */
    if character.len_utf8() != bytes as usize {
        return Err(());
    }

    return Ok(character);
}

pub fn check_utf8_v2(s: &[u8]) -> Result<(), usize> {
    let mut iter = s.array_windows::<4>();
    let mut stop = 0usize;
    let mut bytes = 0usize;

    while let Some(window) = iter.nth(bytes as usize) {
        let dword = u32::from_le_bytes(*window);

        /* ASCII letters can have only 7 bits set, so we check 
         * if any byte has 8th bit set */
        let all_ascii_mask = 0x80808080u32;
        if dword & all_ascii_mask == 0 {
            stop += 4;
            bytes = 3;
            continue;
        }

        bytes = match __check_utf8_v2(dword) {
            Ok(c) => c.len_utf8(),
            Err(()) => return Err(stop),
        };

        stop += bytes;
        bytes -= 1;
    }

    let tail = match s[stop..] {
        [] => [0xFF, 0xFF, 0xFF, 0xFF],
        [a] => [a, 0xFF, 0xFF, 0xFF],
        [a,b] => [a, b, 0xFF, 0xFF],
        [a,b,c] => [a, b, c, 0xFF],
        _ => unreachable!(),
    };

    let mut tail = i32::from_le_bytes(tail);

    while s.len() != stop {
        let len = match __check_utf8_v2(tail as u32) {
            Ok(c) => c.len_utf8(),
            Err(()) => return Err(stop),
        };

        tail >>= len * 8;
        stop += len;
    }

    return Ok(());
}

pub fn __check_utf16(packed_codepoints: u32) -> Result<char, ()> {
    let first_word = packed_codepoints as u16;
    let other_word = packed_codepoints >> 16;

    if let Some(c) = char::from_u32(first_word as u32) {
        return Ok(c);
    }
    if !(0xD800..=0xDBFF).contains(&first_word) {
        return Err(());
    }
    if !(0xDC00..0xE000).contains(&other_word) {
        return Err(());
    }

    let mask = 0x3FFu32; // Mask to extract 10 first bits
    let first_bits = first_word as u32 & mask;
    let character = (first_bits << 10) | (other_word & mask);

    /* Adding makes rustc think we go OOB and the following char::from_u32
     * is not a noop, so instead we just OR */
    let character = character | 0x10000;

    return Ok(char::from_u32(character).unwrap());
}

fn u32_from_le_words(x: [u16; 2]) -> u32 {
    let a = x[0] as u32;
    let b = x[1] as u32;
    (a << 0) | (b << 16)
}

pub fn check_utf16(s: &[u16]) -> Result<(), usize> {
    let mut iter = s.array_windows::<2>();
    let mut stop = 0usize;
    let mut bytes = 0usize;

    while let Some(window) = iter.nth(bytes as usize) {
        let dword = u32_from_le_words(*window);

        let mask = 0xF800_F800u32;
        if dword & mask != 0xD800_D800u32 {
            stop += 2;
            bytes = 1;
            continue;
        }

        bytes = match __check_utf16(dword) {
            Ok(c) => c.len_utf16(),
            Err(()) => return Err(stop),
        };

        stop += bytes;
        bytes -= 1;
    }

    if stop == s.len() {
        return Ok(());
    }

    if let Some(last) = s.last() {
        let dword = u32_from_le_words([*last, 0xD800]);
        match __check_utf16(dword) {
            Ok(_) => return Ok(()),
            Err(()) => return Err(stop),
        }
    }

    return Ok(());
}

