//! ISO-8601 date/time and Allen interval-algebra operations (TerminusDB 12).
//!
//! Intervals are `xdd:dateTimeInterval` values (half-open). `IntervalRelation`
//! and `IntervalRelationTyped` classify or validate the 13 Allen relations:
//! `before, after, meets, met_by, overlaps, overlapped_by, starts, started_by,
//! during, contains, finishes, finished_by, equals`.

use crate::prelude::*;
use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Construct or deconstruct an `xdd:dateTimeInterval` from a start and end point.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Interval {
    /// The start point of the interval.
    pub start: DataValue,
    /// The end point of the interval.
    pub end: DataValue,
    /// The resulting (or supplied) interval.
    pub interval: DataValue,
}

/// Construct or deconstruct an interval from a start point and a duration.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct IntervalStartDuration {
    /// The start point of the interval.
    pub start: DataValue,
    /// The duration of the interval.
    pub duration: DataValue,
    /// The resulting (or supplied) interval.
    pub interval: DataValue,
}

/// Construct or deconstruct an interval from a duration and an end point.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct IntervalDurationEnd {
    /// The duration of the interval.
    pub duration: DataValue,
    /// The end point of the interval.
    pub end: DataValue,
    /// The resulting (or supplied) interval.
    pub interval: DataValue,
}

/// Classify or validate the Allen relation between two intervals given as four endpoints.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct IntervalRelation {
    /// The Allen relation (e.g. "before", "meets", "overlaps", ...).
    pub relation: DataValue,
    /// The start of the first interval.
    pub x_start: DataValue,
    /// The end of the first interval.
    pub x_end: DataValue,
    /// The start of the second interval.
    pub y_start: DataValue,
    /// The end of the second interval.
    pub y_end: DataValue,
}

/// Classify or validate the Allen relation between two `xdd:dateTimeInterval` values.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct IntervalRelationTyped {
    /// The Allen relation (e.g. "before", "meets", "overlaps", ...).
    pub relation: DataValue,
    /// The first interval.
    pub x: DataValue,
    /// The second interval.
    pub y: DataValue,
}

/// Tri-directional, end-of-month-preserving date/duration arithmetic.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DateDuration {
    /// The start date.
    pub start: DataValue,
    /// The duration between start and end.
    pub duration: DataValue,
    /// The end date.
    pub end: DataValue,
}

/// The day after a given date.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DayAfter {
    /// The input date.
    pub date: DataValue,
    /// The next day.
    pub next: DataValue,
}

/// The day before a given date.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct DayBefore {
    /// The input date.
    pub date: DataValue,
    /// The previous day.
    pub previous: DataValue,
}

/// The ISO week number and ISO week-based year of a date.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct IsoWeek {
    /// The input date.
    pub date: DataValue,
    /// The ISO week number.
    pub week: DataValue,
    /// The ISO week-based year.
    pub year: DataValue,
}

/// The weekday of a date (ISO: Monday = 1).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct Weekday {
    /// The input date.
    pub date: DataValue,
    /// The weekday number.
    pub weekday: DataValue,
}

/// The weekday of a date with Sunday-based numbering (Sunday = 1).
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct WeekdaySundayStart {
    /// The input date.
    pub date: DataValue,
    /// The weekday number (Sunday = 1).
    pub weekday: DataValue,
}

/// The first date of the month given by a year-month.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct MonthStartDate {
    /// The year and month (e.g. an xsd:gYearMonth).
    pub year_month: DataValue,
    /// The first date of that month.
    pub date: DataValue,
}

/// The last date of the month given by a year-month.
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct MonthEndDate {
    /// The year and month (e.g. an xsd:gYearMonth).
    pub year_month: DataValue,
    /// The last date of that month.
    pub date: DataValue,
}

/// Generate month start dates within the range ['start', 'end').
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct MonthStartDates {
    /// The generated month-start date.
    pub date: DataValue,
    /// The inclusive start of the range.
    pub start: DataValue,
    /// The exclusive end of the range.
    pub end: DataValue,
}

/// Generate month end dates within the range ['start', 'end').
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]

pub struct MonthEndDates {
    /// The generated month-end date.
    pub date: DataValue,
    /// The inclusive start of the range.
    pub start: DataValue,
    /// The exclusive end of the range.
    pub end: DataValue,
}
