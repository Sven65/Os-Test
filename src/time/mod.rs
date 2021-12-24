pub mod cmos;

use crate::time::cmos::{CMOS, CMOSCenturyHandler, RTCDateTime};

pub fn get_time_with_year(year: Option<i8>) -> RTCDateTime {
	let mut cmos = unsafe { CMOS::new() };

	let rtc = cmos.read_rtc(CMOSCenturyHandler::CurrentYear(2021));

	rtc
}

pub fn get_time() -> RTCDateTime {
	let mut cmos = unsafe { CMOS::new() };

	let rtc = cmos.read_rtc(CMOSCenturyHandler::CenturyRegister(32));

	rtc
}