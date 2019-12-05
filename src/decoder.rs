
const START_MARKER_UTF8_Q: &'static [&'static str] = &["=?UTF-8?q?", "=?utf-8?q?"];
const START_MARKER_UTF8_B: &'static [&'static str] = &["=?UTF-8?B?"];
const END_MARKER_UTF8: &'static [&'static str] = &["?="];

pub fn decode(field: String) -> String {
    let mut changed: Vec<((usize, usize), String)> = Vec::new();

    let codes: Vec<(&[& str], & [& str], Box<(dyn Fn(&str) -> String)>)> = vec![
        ( START_MARKER_UTF8_Q, END_MARKER_UTF8, Box::new(decode_utf8_q) ),
        ( START_MARKER_UTF8_B, END_MARKER_UTF8, Box::new(decode_utf8_b) ), ];

    let mut start = 0;
    let mut end = 1;
    let length = field.len();

    while end <= length {
        for (start_markers, end_markers, decode_fn) in codes.iter() {
            if let Some(size) = match_marker(&field[start..end], start_markers) {
                let inner_min = end;
                let outer_min = inner_min - size;
                let (inner_max, outer_max) = match find_marker(&field[end..], end_markers) {
                    Some((offset, marker_len)) => (inner_min + offset, inner_min + offset + marker_len),
                    None => (length, length),
                };
                // Decode inner
                let decoded = decode_fn(&field[inner_min..inner_max]);
                changed.push( ( (outer_min, outer_max), decoded ) );

                // Set indices to new vals
                start = outer_max;
                end = outer_max;
                break;
            }
        }
        end += 1;
    }

    // Insert changed into string
    changed.sort_by(|(a, _), (b, _)| {
        a.cmp(b)
    });

    let mut ret = String::new();

    let mut index = 0;
    for ((start, end), s) in changed.into_iter() {
        if index < start {
            ret.push_str(&field[index..start]);
        }
        ret.push_str(s.as_str());
        index = end;
    }
    ret.push_str(&field[index..]);

    return ret;
}

fn match_marker(s: &str, markers: &[&str]) -> Option<usize> {
    for marker in markers.iter() {
        if s.ends_with(marker) {
            return Some(marker.len());
        }
    }
    return None;
}

fn find_marker(field: &str, markers: &[&str]) -> Option<(usize, usize)> {
    let mut end = 1;

    let length = field.len();
    while end <= length {
        for marker in markers.iter() {
            if field[..end].ends_with(marker) {
                let len = marker.len();
                return Some((end - len, len));
            }
        }
        end += 1;
    }
    return None;
}

fn decode_utf8_q(field: &str) -> String {
    let mut buf = String::new();

    let mut processing_hex = false;
    let mut utf8_buf = String::new();

    for c in field.chars() {
        if processing_hex {
            // If buf length lt 2, add char, otherwise push as one byte
            utf8_buf.push(c);
            if utf8_buf.len() == 2 {
                let byte = u8::from_str_radix(utf8_buf.as_str(), 16).unwrap();
                buf.push_str(String::from_utf8(vec![byte]).unwrap_or_default().as_str());
                utf8_buf.clear();
                processing_hex = false;
            }
        } else {
            if c == '=' {
                processing_hex = true;
            } else if c == '_' {
                buf.push(' ');
            } else {
                buf.push(c);
            }
        }
    }
    return buf;
}

fn decode_utf8_b(s: &str) -> String {
    String::from_utf8(base64::decode(s).unwrap_or(Vec::new())).unwrap_or(String::new())
}

use datetime::{
    Offset,
    OffsetDateTime,
    Month,
    LocalDate,
    LocalTime,
    LocalDateTime,
};

pub fn decode_date(s: &str) -> Option<OffsetDateTime> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    // format "Wed, 04 Dec 2019 10:2:8 +0000"
    let monthdays = match tokens[1].parse::<i8>() {
        Ok(val) => val,
        Err(_) => return None,
    };
    let month = Month::from_one(match tokens[2].to_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    }).unwrap();
    let year = match tokens[3].parse::<i64>() {
        Ok(val) => val,
        Err(_) => return None,
    };
    let time_tokens: Vec<&str> = tokens[4].split_terminator(':').collect();
    let hour = match time_tokens[0].parse::<i8>() {
        Ok(val) => val,
        Err(_) => return None,
    };
    let minute = match time_tokens[1].parse::<i8>() {
        Ok(val) => val,
        Err(_) => return None,
    };
    let second = match time_tokens[2].parse::<i8>() {
        Ok(val) => val,
        Err(_) => return None,
    };
    let offset = match tokens[5].parse::<i64>() {
        Ok(val) => val,
        Err(_) => return None,
    };

    let date = LocalDate::ymd(year, month, monthdays).unwrap();
    let time = LocalTime::hms(hour, minute, second).unwrap();
    let datetime = LocalDateTime::new(date, time);
    let offset = Offset::of_hours_and_minutes((offset / 100) as i8, (offset % 100) as i8).unwrap();
    Some(offset.transform_date(datetime))
}
