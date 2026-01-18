pub fn decompress(buffer: &[u8]) -> Vec<u8> {
    let mut k: u32 = 7;
    let mut val: u32 = 0;
    let mut l: usize = 0;

    // Preallocate with capacity
    let mut raw_bytes = Vec::with_capacity(buffer.len() * 10);

    // Use vectors instead of fixed arrays
    let mut int_odd = vec![0u32; 256];
    let mut int_even = vec![0u32; 256];
    let mut byte_pair = vec![0u8; 513];

    // Initialize lookup tables
    for i in 0..256 {
        int_odd[i] = 2 * i as u32 + 1;
        int_even[i] = 2 * i as u32 + 2;
        byte_pair[i * 2 + 1] = i as u8;
        byte_pair[i * 2 + 2] = i as u8;
    }

    while val != 0x100 {
        val = 0;

        while val <= 0xFF {
            if k == 7 {
                l += 1;
                k = 0;
            } else {
                k += 1;
            }

            // Optimize bit manipulation
            val = if (buffer[4 + l - 1] & (1 << k)) != 0 {
                int_even[val as usize]
            } else {
                int_odd[val as usize]
            };
        }

        let mut val3 = val;
        let mut val2 = byte_pair[val as usize] as u32;

        while val3 != 0 && val2 != 0 {
            let i = byte_pair[val2 as usize] as u32;
            let mut j = int_odd[i as usize];

            if j == val2 {
                j = int_even[i as usize];
                int_even[i as usize] = val3;
            } else {
                int_odd[i as usize] = val3;
            }

            if int_odd[val2 as usize] == val3 {
                int_odd[val2 as usize] = j;
            } else {
                int_even[val2 as usize] = j;
            }

            byte_pair[val3 as usize] = i as u8;
            byte_pair[j as usize] = val2 as u8;
            val3 = i;
            val2 = byte_pair[val3 as usize] as u32;
        }

        val = val.wrapping_add(0xFFFFFF00);

        if val == 0x100 {
            continue;
        }

        raw_bytes.push(val as u8);
    }

    raw_bytes
}
