//! Real-Time Clock (RTC) Driver
//! Reads time from CMOS RTC

use crate::util::{inb, outb};

const CMOS_ADDRESS: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

unsafe fn read_cmos(reg: u8) -> u8 {
    outb(CMOS_ADDRESS, reg);
    inb(CMOS_DATA)
}

unsafe fn bcd_to_binary(bcd: u8) -> u8 {
    (bcd / 16) * 10 + (bcd & 0x0F)
}

pub fn read_rtc() -> DateTime {
    unsafe {
        // Wait for RTC update to complete
        while read_cmos(0x0A) & 0x80 != 0 {}
        
        let second = read_cmos(0x00);
        let minute = read_cmos(0x02);
        let hour = read_cmos(0x04);
        let day = read_cmos(0x07);
        let month = read_cmos(0x08);
        let year = read_cmos(0x09);
        
        // Check if RTC is in BCD mode (bit 2 of register B)
        let register_b = read_cmos(0x0B);
        let is_bcd = register_b & 0x04 == 0;
        
        DateTime {
            second: if is_bcd { bcd_to_binary(second) } else { second },
            minute: if is_bcd { bcd_to_binary(minute) } else { minute },
            hour: if is_bcd { bcd_to_binary(hour) } else { hour },
            day: if is_bcd { bcd_to_binary(day) } else { day },
            month: if is_bcd { bcd_to_binary(month) } else { month },
            year: 2000 + if is_bcd { bcd_to_binary(year) } else { year } as u16,
        }
    }
}

pub fn format_time() -> alloc::string::String {
    use alloc::format;
    let dt = read_rtc();
    format!("{:02}:{:02}", dt.hour, dt.minute)
}
