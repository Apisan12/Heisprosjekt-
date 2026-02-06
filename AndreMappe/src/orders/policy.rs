// pub fn nearest(
//     orders: &[bool],
//     current_floor: u8,
// ) -> Option<u8> {
//     orders.iter()
//         .enumerate()
//         .filter(|(_, &o)| o)
//         .map(|(i, _)| i as u8)
//         .min_by_key(|f| (*f as i16 - current_floor as i16).abs())
// }
