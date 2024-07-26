use color_eyre::eyre::Error;
use time::{Duration, OffsetDateTime, Time, UtcOffset, Weekday};

/// Determine the last monday.
/// If today is monday, returns today.
/// If today is tuesday, returns yesterday.
/// Time is set to midnight.
pub fn last_monday(offset: UtcOffset) -> OffsetDateTime {
    let now = OffsetDateTime::now_utc().to_offset(offset);

    let date = now.date();
    let last_monday = match now.weekday() {
        Weekday::Monday => date,
        Weekday::Tuesday => date - Duration::days(1),
        Weekday::Wednesday => date - Duration::days(2),
        Weekday::Thursday => date - Duration::days(3),
        Weekday::Friday => date - Duration::days(4),
        Weekday::Saturday => date - Duration::days(5),
        Weekday::Sunday => date - Duration::days(6),
    };

    last_monday.midnight().assume_offset(offset)
}

/// Return the start and end dates for the Pretix export.
/// The provided `monday` indicates the start of the export period,
/// the end date will be the first sunday following the provided monday.
///
/// The time of both dates will be midnight.
///
/// # Errors
///
/// If `monday` is not actually a monday.
pub fn pretix_export_period(
    monday: OffsetDateTime,
    offset: UtcOffset,
) -> color_eyre::Result<(OffsetDateTime, OffsetDateTime)> {
    if monday.weekday().ne(&Weekday::Monday) {
        return Err(Error::msg(format!(
            "The 'Monday' provided is not actually a monday, but a {}",
            monday.weekday()
        )));
    }

    Ok((
        monday
            .date()
            .with_time(Time::MIDNIGHT)
            .assume_offset(offset),
        (monday.date() + Duration::days(6))
            .with_time(Time::MIDNIGHT)
            .assume_offset(offset),
    ))
}
