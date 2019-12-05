pub fn fit_string_to_size(input: &String, size: usize) -> String {
    let mut s = input.clone();
    if s.len() > size {
        while s.len() > (size - 4) {
            s.pop();
        }
        s.push_str(" ...");
    } else if s.len() < size {
        while s.len() < size {
            s.push(' ');
        }
    }
    return s;
}

use datetime::{
    OffsetDateTime,
    DatePiece,
    TimePiece,
};

pub fn format_date(date: &OffsetDateTime) -> String {
    format!("{:0>2}.{:0>2}.{}, {:0>2}:{:0>2}:{:0>2}", date.day(), date.month().months_from_january() + 1, date.year(), date.hour(), date.minute(), date.second())
}

use std::cmp::Ordering;

pub fn compare_date(date0: &OffsetDateTime, date1: &OffsetDateTime) -> Ordering {
    let fields0 = get_timestamp_fields(&date0);
    let fields1 = get_timestamp_fields(&date1);

    let mut level = 0;
    let mut result = Ordering::Equal;
    while let Ordering::Equal = result {
        if level >= fields0.len() {
            break;
        }
        result = fields0[level].cmp(&fields1[level]);
        level += 1;
    }
    return result;
}

fn get_timestamp_fields(datetime: &OffsetDateTime) -> Vec<i64> {
    vec![
    datetime.year(),
    datetime.month().months_from_january() as i64,
    datetime.day() as i64,
    datetime.hour() as i64,
    datetime.minute() as i64,
    datetime.minute() as i64,
    datetime.second() as i64,
    datetime.millisecond() as i64
    ]
}
